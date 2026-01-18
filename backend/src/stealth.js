
// Advanced Stealth Payload for Cloudflare Bypass

// 1. Overwrite navigator.webdriver
Object.defineProperty(navigator, 'webdriver', {
    get: () => undefined,
});

// 2. Mock navigator.plugins to look like a normal browser
Object.defineProperty(navigator, 'plugins', {
    get: () => {
        const p = [
            { name: "Chrome PDF Plugin", filename: "internal-pdf-viewer", description: "Portable Document Format" },
            { name: "Chrome PDF Viewer", filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai", description: "" },
            { name: "Native Client", filename: "internal-nacl-plugin", description: "" }
        ];
        // Add fake iterator/item methods
        p.item = (i) => p[i];
        p.namedItem = (name) => p.find(x => x.name === name);
        p.refresh = () => { };
        return p;
    },
});

// 3. Mock languages
Object.defineProperty(navigator, 'languages', {
    get: () => ['en-US', 'en', 'ko-KR', 'ko'],
});

// 4. Mock window.chrome
if (!window.chrome) {
    window.chrome = {
        runtime: {},
        loadTimes: function () { },
        csi: function () { },
        app: {}
    };
}

// 5. Mock Permissions API to pass 'notifications' check often used by bots
const originalQuery = window.navigator.permissions.query;
window.navigator.permissions.query = (parameters) => (
    parameters.name === 'notifications' ?
        Promise.resolve({ state: Notification.permission }) :
        originalQuery(parameters)
);

// 6. WebGL Vendor Spoofing (Optional but helpful)
const getParameter = WebGLRenderingContext.prototype.getParameter;
WebGLRenderingContext.prototype.getParameter = function (parameter) {
    // UNMASKED_VENDOR_WEBGL
    if (parameter === 37445) {
        return 'Intel Inc.';
    }
    // UNMASKED_RENDERER_WEBGL
    if (parameter === 37446) {
        return 'Intel(R) Iris(R) Xe Graphics';
    }
    return getParameter(parameter);
};
