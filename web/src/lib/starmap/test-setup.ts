// The seam tests run the island headless — no 2D context, nothing drawn. jsdom
// has no canvas backend and logs a "not implemented" error on getContext; stub
// it to quietly return null (the island's own graceful-degradation path) so the
// test output stays clean.
// Derived from chartr (https://github.com/rengwu/chartr), MIT, Copyright (c) 2026 John Goh.

HTMLCanvasElement.prototype.getContext = (() => null) as never
