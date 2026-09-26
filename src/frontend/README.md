# src/frontend

The **frontend shell** of PixivFlow Desktop.

At **F0 (Foundation)** this directory only holds a static `index.html` shell page
and this README — no app logic, no build tooling.

From **Phase 1** onward the real user interface is the bundled **pixivflow-webui**
SPA, served same-origin by the local PixivFlow backend and rendered in the Tauri
window. This `index.html` remains only as a fallback / error shell when the
backend is not (yet) healthy.

## F0 note

Do **not** initialize a build system here at F0. The webui integration belongs to
Phase 1 (`Shell MVP`) and Phase 2 (`Backend integration`).