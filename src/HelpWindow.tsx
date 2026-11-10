/**
 * HelpWindow — "Help & About…": the tray's small reference window.
 *
 * Shows the runtime app version (from `@tauri-apps/api/app`, never hardcoded),
 * the author links, the project docs, and the acknowledgements. NOTICE is the
 * source of truth: its raw text is rendered as-is at build time (`?raw`), so
 * the help window can never drift from the file. External links go through the
 * opener plugin — this window never navigates in-webview.
 */
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import notice from "../NOTICE?raw";
import "./HelpWindow.css";

const GITHUB = "https://github.com/glebis";
const LINKEDIN = "https://www.linkedin.com/in/glebkalinin/";
const REPO = "https://github.com/glebis/cenno";
const README = "https://github.com/glebis/cenno#readme";
const CONFIG_DOC = "https://github.com/glebis/cenno/blob/main/docs/CONFIG.md";
const CONTROLS_DOC =
  "https://github.com/glebis/cenno/blob/main/docs/design/CONTROLS.md";

export default function HelpWindow() {
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {
        /* non-Tauri context (tests/preview) — leave the version blank */
      });
  }, []);

  return (
    <div className="hw">
      <header className="hw__top">
        <div className="hw__brand">
          <CennoMark />
          <span className="hw__title">cenno</span>
          {version && <span className="hw__version">v{version}</span>}
        </div>
      </header>

      <main className="hw__body">
        <section className="hw__section">
          <h2>What is cenno?</h2>
          <p className="hw__muted">
            A macOS menu-bar app that lets MCP-capable AI agents ask you
            questions through small floating panels — without stealing keyboard
            focus. The agent calls <code>ask_user</code>, a panel slides in, and
            your answer comes back as structured data. Every exchange stays
            local.
          </p>
        </section>

        <section className="hw__section">
          <h2>Author</h2>
          <p className="hw__muted">Made by Gleb Kalinin.</p>
          <ul className="hw__links">
            <li>
              <button className="hw__link" onClick={() => openUrl(GITHUB)}>
                github.com/glebis →
              </button>
            </li>
            <li>
              <button className="hw__link" onClick={() => openUrl(LINKEDIN)}>
                linkedin.com/in/glebkalinin →
              </button>
            </li>
          </ul>
        </section>

        <section className="hw__section">
          <h2>Docs & project</h2>
          <ul className="hw__links">
            <li>
              <button className="hw__link" onClick={() => openUrl(REPO)}>
                cenno on GitHub →
              </button>
            </li>
            <li>
              <button className="hw__link" onClick={() => openUrl(README)}>
                README — install, usage, releasing →
              </button>
            </li>
            <li>
              <button className="hw__link" onClick={() => openUrl(CONFIG_DOC)}>
                Configuration reference (~/.cenno) →
              </button>
            </li>
            <li>
              <button className="hw__link" onClick={() => openUrl(CONTROLS_DOC)}>
                Controls & flows catalog →
              </button>
            </li>
          </ul>
        </section>

        <section className="hw__section">
          <h2>Acknowledgements</h2>
          <p className="hw__muted">
            cenno is Apache-2.0 licensed. It was built with AI assistance under
            human direction, and stands on open-source work. The full notice,
            kept at <code>NOTICE</code> in the repository, is reproduced below.
          </p>
          <pre className="hw__notice">{notice.trim()}</pre>
        </section>
      </main>

      <footer className="hw__footer">
        <span>cenno{version ? ` v${version}` : ""}</span>
        <span>Apache-2.0 · Gleb Kalinin</span>
      </footer>
    </div>
  );
}

// The cenno wordmark glyph from the site: an arc over a dot (a head nodding).
function CennoMark() {
  return (
    <svg className="hw__mark" width="22" height="22" viewBox="0 0 22 22" aria-hidden="true">
      <path
        d="M 5.6874 7.5765 A 5.5 5.5 0 0 1 16.3126 7.5765"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="butt"
      />
      <circle cx="11" cy="16.25" r="2.75" fill="currentColor" />
    </svg>
  );
}
