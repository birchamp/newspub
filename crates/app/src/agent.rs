//! The desktop app as an MCP host (AG-02): `newpub --agent` serves the protocol on stdin/stdout while the window
//! shows the publication, so a person watches an agent work and can take over at any time. The tools and their
//! dispatch live in `newpub-agent`; this file only keeps the view in step.

use std::sync::{Arc, Mutex};

use newpub_agent::{Host, Link, Waker};

use crate::NewpubApp;

/// A live stdio connection: the reader thread queues requests and wakes the UI, which answers them each frame.
pub struct Live {
    link: Link,
    wake: Waker,
}

impl Host for NewpubApp {
    fn session(&mut self) -> &mut newpub_engine::Session {
        &mut self.session
    }

    /// After an agent changed the document: leave text editing (the caret may point at text that is gone), keep
    /// the page and selection valid, and tell the person what happened in the status bar.
    fn after_actions(&mut self, summary: &str) {
        self.end_text_edit();
        self.sync_after_change();
        self.status = summary.to_string();
    }
}

impl NewpubApp {
    /// Starts serving MCP on stdin/stdout (the `--agent` flag).
    pub fn attach_agent(&mut self) {
        let wake: Waker = Arc::new(Mutex::new(None));
        let link = newpub_agent::stdio_link(wake.clone());
        self.agent = Some(Live { link, wake });
    }

    /// Answers the requests an agent queued since the last frame.
    pub(crate) fn agent_pump(&mut self, ctx: &egui::Context) {
        let Some(live) = self.agent.take() else { return };
        if let Ok(mut w) = live.wake.lock()
            && w.is_none()
        {
            let c = ctx.clone();
            *w = Some(Box::new(move || c.request_repaint()));
        }
        if live.link.pump(self) > 0 {
            ctx.request_repaint();
        }
        self.agent = Some(live);
    }
}
