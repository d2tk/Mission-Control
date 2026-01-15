# Implementation Plan: Virtual AI Chat Room (No APIs)

## Goal Description
Create a "Virtual Chat Room" (a local web interface) where the User, Antigravity, and "Bot Agents" (ChatGPT, Claude, Grok) can converse.
Since API keys are excluded, the "Bot Agents" will be powered by **Browser Automation**. A background script will read the chat room, copy new messages, paste them into the respective AI's web interface (opened in a browser), and copy the response back to the chat room.

## User Review Required
> [!WARNING]
> **Fragility & Complexity**: Browser automation is significantly more complex and fragile than APIs.
> - **Login Required**: You must be logged into these services in the browser controlled by the automation script.
> - **Breakage**: Changes to the website UI (ChatGPT/Claude updates) will break the integration.
> - **Rate Limits**: Web interfaces have stricter rate limits and CAPTCHAs.

> [!CAUTION]
> **Anti-Bot Detection**: Sites like ChatGPT use Cloudflare to detect automation.
> - We use `disable-blink-features=AutomationControlled` to hide the most obvious flags.
> - If a CAPTCHA/Challenge appears, the user **MUST** manually solve it in the browser window opened by the script.

## Proposed Architecture

### 1. The "Virtual Room" (Backend)
A simple file-based database (e.g., `chat_history.json`) that acts as the single source of truth for the conversation.
- **Structure**: `[{ "sender": "User", "text": "Hi" }, { "sender": "ChatGPT", "text": "Hello" }]`

### 2. The Chat UI (Frontend)
A minimal HTML/CSS/JS page to visualize the `chat_history.json`.
- Auto-refreshes to show new messages.
- Input box to let the User (or Me, acting via file edits) add messages.

### 3. The Automation Bridge ("The Runners")
Python scripts using **Playwright** (robust browser automation).
- **Architecture**: **Non-blocking concurrent tasks**. Each agent request is handled in a separate `asyncio.Task` to prevent one agent's failure or long wait from blocking others.
- **Role**:
    1. Watch `chat_history.json` for new messages addressed to them (e.g., "@ChatGPT ...").
    2. Open the web page (e.g., chatgpt.com).
    3. Input the prompt.
    4. Wait for generation (90s).
    5. Scrape the result using text diffing.
    6. Append to `chat_history.json`.
- **Stealth**: Modified to hide `navigator.webdriver` and use stealth arguments.
- **Grok Strategy**: Use `click()` followed by `keyboard.type()` to bypass `contenteditable` limitations.

## Verification Plan

### Manual Verification
1.  **UI Test**: Start the Chat UI and ensure typing updates `chat_history.json`.
2.  **Runner Test**: Run the Playwright script. Manually verify it creates a browser instance, navigates to the AI site, and uses existing cookies/login session (using a persistent browser context).
