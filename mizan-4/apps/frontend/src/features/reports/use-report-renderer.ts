import { useCallback, useRef } from "react";
import type { ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import html2canvas from "html2canvas";
import { jsPDF } from "jspdf";

import { A4_HEIGHT_PX, A4_WIDTH_PX } from "./report-shell";

/**
 * The capture scale fed to html2canvas. 2 keeps text crisp at print scale
 * without ballooning the resulting PDF (~3-5 MB for a typical 4-page report).
 */
const CAPTURE_SCALE = 2;

/**
 * jsPDF works in millimetres for A4. 1 px @ 96 DPI ≈ 0.2645833 mm.
 * We pin to that ratio so the rendered canvas drops in 1:1.
 */
const PX_TO_MM = 25.4 / 96;
const A4_WIDTH_MM = A4_WIDTH_PX * PX_TO_MM;
const A4_HEIGHT_MM = A4_HEIGHT_PX * PX_TO_MM;

export interface RenderResult {
  blob: Blob;
  /**
   * Suggested filename including extension. Callers pass `filename` in to
   * customise; otherwise we derive from the title slug + ISO date.
   */
  filename: string;
}

export interface RenderOptions {
  /** The React tree to render — typically a `<ReportShell>` with sections. */
  tree: ReactNode;
  /** Used to build the default filename. */
  title: string;
  /** Override the derived filename if you have one. */
  filename?: string;
}

/**
 * Hook that renders a React tree off-screen, captures it with html2canvas,
 * and paginates the result into a multi-page A4 PDF Blob.
 *
 * Why off-screen instead of inline:
 * - The report layout is fixed at 794 px wide (A4) and must not be affected
 *   by the live page's CSS, scroll position, or dark-mode tokens.
 * - html2canvas requires the node to be in the document; a detached node
 *   captures as blank. We attach to body with `position: fixed; left: -9999px`.
 *
 * The hook returns a `render` callback. It is stable across renders so
 * passing it as a prop won't trigger downstream re-renders.
 */
export function useReportRenderer() {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const rootRef = useRef<Root | null>(null);

  const render = useCallback(async (opts: RenderOptions): Promise<RenderResult> => {
    const container = document.createElement("div");
    container.setAttribute("data-mizan-report-container", "");
    container.style.position = "fixed";
    container.style.top = "0";
    container.style.left = "-99999px";
    container.style.width = `${A4_WIDTH_PX}px`;
    container.style.pointerEvents = "none";
    container.style.background = "#ffffff";
    document.body.appendChild(container);
    containerRef.current = container;

    const root = createRoot(container);
    rootRef.current = root;

    try {
      // Mount and wait one paint so fonts/images settle before capture.
      root.render(opts.tree);
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

      // Capture the entire scrollHeight at A4 width.
      const canvas = await html2canvas(container, {
        scale: CAPTURE_SCALE,
        backgroundColor: "#ffffff",
        useCORS: true,
        // Disable taint guard — branding logos are same-origin or CORS-able.
        allowTaint: false,
        windowWidth: A4_WIDTH_PX,
      });

      // Paginate: slice the tall canvas into A4_HEIGHT_PX × CAPTURE_SCALE
      // bands and place each on a new PDF page.
      const pdf = new jsPDF({
        orientation: "portrait",
        unit: "mm",
        format: "a4",
      });

      const pageHeightPx = A4_HEIGHT_PX * CAPTURE_SCALE;
      const totalHeightPx = canvas.height;
      let offsetPx = 0;
      let pageIndex = 0;

      while (offsetPx < totalHeightPx) {
        const sliceHeightPx = Math.min(pageHeightPx, totalHeightPx - offsetPx);

        // Draw the slice onto a per-page canvas.
        const pageCanvas = document.createElement("canvas");
        pageCanvas.width = canvas.width;
        pageCanvas.height = sliceHeightPx;
        const ctx = pageCanvas.getContext("2d");
        if (!ctx) {
          throw new Error("Failed to get 2D context for report pagination");
        }
        ctx.fillStyle = "#ffffff";
        ctx.fillRect(0, 0, pageCanvas.width, pageCanvas.height);
        ctx.drawImage(
          canvas,
          0,
          offsetPx,
          canvas.width,
          sliceHeightPx,
          0,
          0,
          canvas.width,
          sliceHeightPx,
        );

        const sliceHeightMm = (sliceHeightPx / CAPTURE_SCALE) * PX_TO_MM;
        const dataUrl = pageCanvas.toDataURL("image/jpeg", 0.92);

        if (pageIndex > 0) {
          pdf.addPage("a4", "portrait");
        }
        pdf.addImage(dataUrl, "JPEG", 0, 0, A4_WIDTH_MM, sliceHeightMm);

        offsetPx += sliceHeightPx;
        pageIndex += 1;

        // Guard against infinite loops if pagination math drifts.
        if (pageIndex > 50) {
          throw new Error("Report exceeded 50 pages — refusing to render");
        }
      }

      const blob = pdf.output("blob");
      const slug = opts.title
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "");
      const date = new Date().toISOString().slice(0, 10);
      const filename = opts.filename ?? `${slug || "mizan-report"}-${date}.pdf`;

      void A4_HEIGHT_MM; // referenced indirectly via page sizing — silence unused warning if elided
      return { blob, filename };
    } finally {
      // Always unmount + detach, even on error, to avoid leaking nodes.
      try {
        rootRef.current?.unmount();
      } catch {
        // Ignore unmount errors — best-effort cleanup.
      }
      rootRef.current = null;
      if (containerRef.current?.parentNode) {
        containerRef.current.parentNode.removeChild(containerRef.current);
      }
      containerRef.current = null;
    }
  }, []);

  /**
   * Convenience: render + trigger a browser download. Callers in Tauri
   * environments should prefer `render()` directly and route through
   * Tauri's `save` dialog.
   */
  const downloadBrowser = useCallback(
    async (opts: RenderOptions): Promise<RenderResult> => {
      const result = await render(opts);
      const url = URL.createObjectURL(result.blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = result.filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      // Defer revoke so the click handler has time to dispatch.
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      return result;
    },
    [render],
  );

  return { render, downloadBrowser };
}
