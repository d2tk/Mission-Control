# AI Communication Bridge Tasks

- [ ] **Phase 1: Concept & Design**
    - [x] Define "Virtual Chat Room" data structure (JSON/Markdown based log) <!-- id: 0 -->
    - [x] Design the "Meeting Room" UI (Simple Web Interface) <!-- id: 1 -->
    - [x] Create `implementation_plan.md` for Browser Automation Bridge <!-- id: 2 -->

- [ ] **Phase 1.5: Environment Migration (uv & pnpm)**
    - [x] Migrate Python env to `uv` (init, add playwright, requests) <!-- id: 9 -->
    - [x] Initialize frontend with `pnpm` (package.json) <!-- id: 10 -->
    - [x] Update documentation for new tools <!-- id: 11 -->

- [ ] **Phase 2: The "Meeting Room" (Virtual Space)**
    - [x] Create a shared `conversation_log.json` to act as the "Room" <!-- id: 3 -->
    - [x] Build a simple Chat UI (HTML/JS) to view & type in the room <!-- id: 4 -->

- [ ] **Phase 3: The "Runners" (Browser Automation)**
    - [x] Research `playwright` or `selenium` for accessing web chats <!-- id: 5 -->
    - [x] **Experimental**: Build a script to open ChatGPT/Claude and relay messages from the "Room" <!-- id: 6 -->
    - [x] **Constraint Check**: Verify login persistence (User must be logged in) <!-- id: 7 -->

- [ ] **Phase 3.5: Expanding the Roster**
    - [x] **Grok**: Add `x.com/i/grok` support and stabilize input <!-- id: 12 -->
    - [x] **Antigravity (Gemini)**: Add `gemini.google.com` support to `browser_bridge.py` <!-- id: 13 -->
    - [ ] Test multi-bot conversation (User + ChatGPT + Grok + Antigravity) <!-- id: 14 -->

- [ ] **Phase 6: Protocol Standardization & Persistence**
    - [x] Create standardized `agent_system_instructions.md` <!-- id: 19 -->
    - [ ] Induct all agents with the new ACP protocol <!-- id: 20 -->

- [ ] **Phase 7: Safety & Concurrency Locking**
    - [/] Implement "Agent Busy" state to prevent double-triggering <!-- id: 21 -->
    - [ ] Refine trigger logic to distinguish between "Commands" and "Mentions" <!-- id: 22 -->
    - [ ] Fix Gemini (Antigravity) empty response scraping <!-- id: 23 -->

