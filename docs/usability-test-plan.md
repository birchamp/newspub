# newpub usability test plan

Blocker B-003 in PROGRESS.md needs real people. This kit lets one facilitator run the study in about a week. The journeys prove that every feature works; this study shows whether the people newpub is for can find and use those features.

## 1. Goals

1. Can a first-time user make a common publication (a newsletter, a bulletin or a flyer) from a built-in template without help?
2. Where do people get stuck? Count wrong turns, dead ends and requests for help per task.
3. Do people who know Microsoft Publisher find things where they expect them (menus, shortcuts, dialogs)?
4. Overall perceived usability: the System Usability Scale (SUS), target ≥ 70.

## 2. Participants

- **Who:** 3–5 people from the target users (ARCHITECTURE.md §0): volunteers or staff at small organisations, schools or churches who make newsletters, bulletins, flyers or booklets.
- **Mix:**
  - at least two current or former Publisher users;
  - at least one person who has never used desktop publishing software (Word or Canva only);
  - at least one on each of Windows and macOS;
  - Linux if a participant uses it.
- **Recruiting:** a parish office, a school newsletter editor, a community group's volunteer coordinator. Offer a small thank-you, such as a gift card.
- **Screener questions:**
  1. What do you make, and how often?
  2. Which tools do you use for it today?
  3. Which computer do you use?
  4. Are you comfortable being recorded (screen and voice only)?

## 3. Setup

- **Build:** the release build from the latest green CI run on the participant's own computer, or on a laptop with the same OS. Note the commit hash on the observation sheet.
- **Starting state:**
  - start newpub fresh, with the template picker showing;
  - put the task files in a folder on the desktop:
    - `photo.png` and `stripes.jpg` (from `journeys/fixtures/`);
    - a short article as text (`journeys/fixtures/short.txt`);
    - a 6-row recipient list (`members.csv`).
- **Printer:** use a real printer if one is available. Otherwise use the OS "print to PDF" printer and say so to the participant.
- **Recording:** screen and audio, with consent (section 7). One facilitator and, ideally, one note-taker.
- **Session length:** 60 minutes:
  - 5 minutes introduction;
  - 40 minutes tasks;
  - 10 minutes SUS and debrief;
  - 5 minutes buffer.

## 4. Script

### Introduction (read aloud)

> Thank you for helping. We are testing the software, not you. If something is hard, that is exactly what we need to find out.
> Please think aloud: tell us what you are looking for, what you expect to happen, and what surprises you.
> I will not help during the tasks unless you are completely stuck. If you would give up at home, say "I'd give up here" and we will move on.
> You can stop at any time. Do you agree to the screen and voice recording?

### Tasks

Read each task card aloud and hand it over. Do not use the names of menus or buttons.

| # | Task card | Success when | Time limit |
|---|-----------|--------------|------------|
| T1 | "Start a two-page newsletter for your organisation from one of the ready-made designs." | A newsletter from the template picker is open | 4 min |
| T2 | "Replace the headline with your organisation's name and put this article (short.txt) into the first story." | The headline is changed; the article text is in the story; overflow is handled (continued or autoflow) | 8 min |
| T3 | "Put this photo (photo.png) next to the article, and make the text go around it." | The picture is placed; the text wraps around it | 6 min |
| T4 | "Change the colours of the whole newsletter to something you like better." | A colour scheme is applied, or colours are changed consistently | 4 min |
| T5 | "Save it so you can work on it next week, then save a PDF you could email." | A `.npub` file and a PDF exist where the participant expects them | 5 min |
| T6 | "Print two copies of page 1 only." | The job is sent with copies = 2 and page range = 1 | 4 min |
| T7 | "Make a letter to each person in this list (members.csv) with their name in the greeting." | Merge fields are inserted; the preview shows a recipient's name; the merged publication or PDF is produced | 9 min |

Optional, if time allows: "Make a folded A5 bulletin (a booklet) and print it so it folds in the right order." This tests booklet imposition.

### Facilitator prompts (neutral only)

- "What are you looking for?"
- "What did you expect to happen?"
- "Where would you look next?"
- When stuck for 2 minutes, give a hint ("Have a look in the menus at the top"). When still stuck after 4 minutes, count the task as failed and move on.

### Debrief questions

1. What was the easiest part? The hardest?
2. Was anything missing that you use in your current tool?
3. (Publisher users) What was in a different place from where you expected it?
4. Would you use this for your next newsletter? Why or why not?

## 5. Observation sheet (one per participant)

```
Participant: P__   Date: ________   OS: ________   newpub commit: ________
Background: Publisher user? Y/N   Other tools: ____________________________

Task | Result (S / S-with-hint / F) | Time | Wrong turns (where they looked first) | Quotes / notes | Severity (1-4)
T1   |                              |      |                                       |                |
T2   |                              |      |                                       |                |
T3   |                              |      |                                       |                |
T4   |                              |      |                                       |                |
T5   |                              |      |                                       |                |
T6   |                              |      |                                       |                |
T7   |                              |      |                                       |                |
```

**Severity scale:**
1. Cosmetic.
2. Minor: a short delay, and the participant recovered.
3. Major: a long delay, or a hint was needed.
4. Critical: the task failed, or data was lost.

## 6. SUS questionnaire

Each item is rated 1 (strongly disagree) to 5 (strongly agree).

1. I think that I would like to use this system frequently.
2. I found the system unnecessarily complex.
3. I thought the system was easy to use.
4. I think that I would need the support of a technical person to be able to use this system.
5. I found the various functions in this system were well integrated.
6. I thought there was too much inconsistency in this system.
7. I would imagine that most people would learn to use this system very quickly.
8. I found the system very cumbersome to use.
9. I felt very confident using the system.
10. I needed to learn a lot of things before I could get going with this system.

**Scoring:**
- For odd items, subtract 1 from the rating.
- For even items, subtract the rating from 5.
- Add the ten results and multiply the sum by 2.5, giving a score from 0 to 100. The average is about 68.

## 7. Consent (read and sign)

> I agree to take part in a usability study of newpub. My screen and voice may be recorded. Recordings are used only to
> improve newpub, are not published, and are deleted within 3 months. I can stop at any time without giving a reason.
>
> Name: ____________________   Signature: ____________________   Date: __________

## 8. After the sessions

1. Within a day of each session, transfer the sheets into one table: tasks × participants, with result, time and severity.
2. Group problems by place in the UI, not by participant. Every problem with severity 3 or more, or seen with at least two participants, becomes a backlog item in PROGRESS.md with the participants' quotes.
3. Fixing a backlog item follows the normal rule: the lead first writes a UI journey that reproduces the fixed flow, then the fix is dispatched.
4. Record the SUS mean and range, plus each task's success rate, in REPORT.md.
5. Close B-003 when the study is done and its backlog items are filed.
