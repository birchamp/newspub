//! PDF/X-4 and PDF/UA-1 output (EX-03, AX-03).
//!
//! krilla 0.8 has no PDF/X support, so PDF/X-4 is produced by post-processing the serialized file with lopdf:
//! an OutputIntent (GTS_PDFX) with an embedded CMYK ICC profile, `GTS_PDFXVersion` and `Trapped` in the Info
//! dictionary, an XMP packet carrying `pdfxid:GTS_PDFXVersion`, dates, and a trailer `/ID`.
//!
//! Output profile: `assets/CGATS001Compat-v2-micro.icc`, a CC0 "CGATS TR 001 compatible" CMYK profile from
//! saucecontrol/Compact-ICC-Profiles (the same file the hayro rasteriser ships). The OutputIntent therefore
//! declares the characterised printing condition "CGATS TR 001" (SWOP-like coated stock), which is in the ICC
//! characterisation-data registry. Users targeting FOGRA39 etc. would need a different profile (not bundled).

use crate::{PdfError, PdfOptions, PdfStandard};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use newpub_core::Document;

const CMYK_PROFILE: &[u8] = include_bytes!("../assets/CGATS001Compat-v2-micro.icc");
const CONDITION_ID: &str = "CGATS TR 001";
const CONDITION: &str = "CGATS TR 001 (SWOP), coated stock";

/// Final pass over the serialized PDF for the requested standard.
pub fn finish(bytes: Vec<u8>, doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    match opts.standard {
        None => Ok(bytes),
        Some(PdfStandard::PdfX4) => pdf_x4(bytes, doc),
        Some(s @ PdfStandard::PdfUa1) => Err(PdfError::Unsupported(format!("{s:?} output is not implemented yet"))),
    }
}

/// PDF text string: PDFDocEncoding-safe ASCII as is, anything else UTF-16BE with a BOM.
fn text_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::string_literal(s)
    } else {
        let mut b = vec![0xFE, 0xFF];
        b.extend(s.encode_utf16().flat_map(|u| u.to_be_bytes()));
        Object::string_literal(b)
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// (year, month, day, hour, minute, second) in UTC for a Unix time.
fn utc(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

fn pdf_x4(bytes: Vec<u8>, doc: &Document) -> Result<Vec<u8>, PdfError> {
    let err = |e: lopdf::Error| PdfError::Krilla(format!("PDF/X post-processing: {e}"));
    let mut pdf = lopdf::Document::load_mem(&bytes).map_err(err)?;

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, mo, d, h, mi, s) = utc(now);
    let pdf_date = format!("D:{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z");
    let xmp_date = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z");
    let title = if doc.meta.title.is_empty() { "Untitled" } else { doc.meta.title.as_str() };

    // Output intent with the embedded CMYK profile.
    let mut pdict = Dictionary::new();
    pdict.set("N", 4);
    pdict.set("Alternate", Object::Name(b"DeviceCMYK".to_vec()));
    let profile = pdf.add_object(Stream::new(pdict, CMYK_PROFILE.to_vec()));
    let mut oi = Dictionary::new();
    oi.set("Type", Object::Name(b"OutputIntent".to_vec()));
    oi.set("S", Object::Name(b"GTS_PDFX".to_vec()));
    oi.set("OutputConditionIdentifier", text_string(CONDITION_ID));
    oi.set("OutputCondition", text_string(CONDITION));
    oi.set("RegistryName", text_string("http://www.color.org"));
    oi.set("Info", text_string(CONDITION));
    oi.set("DestOutputProfile", Object::Reference(profile));
    let oi = pdf.add_object(oi);

    // Info dictionary.
    let info_id: ObjectId = match pdf.trailer.get(b"Info").ok().and_then(|o| o.as_reference().ok()) {
        Some(id) => id,
        None => {
            let id = pdf.add_object(Dictionary::new());
            pdf.trailer.set("Info", Object::Reference(id));
            id
        }
    };
    {
        let info = pdf.get_dictionary_mut(info_id).map_err(err)?;
        info.set("Title", text_string(title));
        if !doc.meta.author.is_empty() {
            info.set("Author", text_string(&doc.meta.author));
        }
        info.set("Creator", text_string("newpub"));
        if !info.has(b"Producer") {
            info.set("Producer", text_string("newpub (krilla)"));
        }
        if !info.has(b"CreationDate") {
            info.set("CreationDate", Object::string_literal(pdf_date.clone()));
        }
        info.set("ModDate", Object::string_literal(pdf_date));
        info.set("GTS_PDFXVersion", Object::string_literal("PDF/X-4"));
        info.set("Trapped", Object::Name(b"False".to_vec()));
    }

    // XMP packet (replaces krilla's; it is rebuilt here from the same document metadata).
    let author = if doc.meta.author.is_empty() {
        String::new()
    } else {
        format!("<dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>", xml_escape(&doc.meta.author))
    };
    let xmp = format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\" \
xmlns:pdfxid=\"http://www.npes.org/pdfx/ns/id/\" xmlns:xmpMM=\"http://ns.adobe.com/xap/1.0/mm/\">\n\
<dc:format>application/pdf</dc:format>\n\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\n\
{author}\n\
<xmp:CreatorTool>newpub</xmp:CreatorTool>\n\
<xmp:CreateDate>{xmp_date}</xmp:CreateDate>\n\
<xmp:ModifyDate>{xmp_date}</xmp:ModifyDate>\n\
<xmp:MetadataDate>{xmp_date}</xmp:MetadataDate>\n\
<pdf:Producer>newpub (krilla)</pdf:Producer>\n\
<pdf:Trapped>False</pdf:Trapped>\n\
<pdfxid:GTS_PDFXVersion>PDF/X-4</pdfxid:GTS_PDFXVersion>\n\
<xmpMM:DocumentID>uuid:{uuid}</xmpMM:DocumentID>\n\
<xmpMM:InstanceID>uuid:{uuid}</xmpMM:InstanceID>\n\
</rdf:Description>\n\
</rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>",
        title = xml_escape(title),
        uuid = uuid_like(&bytes, now),
    );
    let mut mdict = Dictionary::new();
    mdict.set("Type", Object::Name(b"Metadata".to_vec()));
    mdict.set("Subtype", Object::Name(b"XML".to_vec()));
    let meta = pdf.add_object(Stream::new(mdict, xmp.into_bytes()));

    let old_meta = {
        let cat = pdf.catalog_mut().map_err(err)?;
        let old = cat.get(b"Metadata").ok().and_then(|o| o.as_reference().ok());
        cat.set("OutputIntents", Object::Array(vec![Object::Reference(oi)]));
        cat.set("Metadata", Object::Reference(meta));
        old
    };
    if let Some(old) = old_meta {
        pdf.objects.remove(&old);
    }

    // File identifiers.
    if pdf.trailer.get(b"ID").is_err() {
        let id = Object::String(uuid_bytes(&bytes, now).to_vec(), lopdf::StringFormat::Hexadecimal);
        pdf.trailer.set("ID", Object::Array(vec![id.clone(), id]));
    }

    let mut out = Vec::with_capacity(bytes.len() + CMYK_PROFILE.len() + 4096);
    pdf.save_to(&mut out).map_err(|e| PdfError::Krilla(format!("PDF/X post-processing: {e}")))?;
    Ok(out)
}

/// 16 identifier bytes derived from the file content and time (FNV-1a, two lanes).
fn uuid_bytes(bytes: &[u8], now: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (lane, chunk) in out.chunks_mut(8).enumerate() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (lane as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for b in bytes.iter().copied().chain(now.to_le_bytes()) {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        chunk.copy_from_slice(&h.to_be_bytes());
    }
    out
}

fn uuid_like(bytes: &[u8], now: u64) -> String {
    let b = uuid_bytes(bytes, now);
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}
