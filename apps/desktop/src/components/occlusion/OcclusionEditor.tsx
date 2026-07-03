import { useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Trash2, ImagePlus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc, isTauri } from "@/lib/ipc";
import { mediaUrl } from "@/lib/renderCard";
import {
  parseOcclusionField,
  serializeOcclusionField,
  type OcclusionShape,
} from "@/lib/imageOcclusion";
import { cn } from "@/lib/utils";

interface DraftBox {
  startX: number;
  startY: number;
  x: number;
  y: number;
}

/**
 * Authoring UI for the "Image Occlusion" stock notetype: pick an image, drag
 * rectangles over it to mark regions, each becomes its own card
 * (`{{c1::…}}`, `{{c2::…}}`, …). Reads/writes the notetype's `Image` and
 * `Occlusion` field HTML directly — no separate data model.
 */
export function OcclusionEditor({
  imageHtml,
  onImageHtmlChange,
  occlusionHtml,
  onOcclusionHtmlChange,
}: {
  imageHtml: string;
  onImageHtmlChange: (html: string) => void;
  occlusionHtml: string;
  onOcclusionHtmlChange: (html: string) => void;
}) {
  const tauri = isTauri();
  const containerRef = useRef<HTMLDivElement>(null);
  const [draft, setDraft] = useState<DraftBox | null>(null);
  const [selected, setSelected] = useState<number | null>(null);

  const shapes = parseOcclusionField(occlusionHtml);
  const filename = imageHtml.match(/src="([^"]+)"/)?.[1] ?? null;
  const displaySrc = filename ? (tauri ? mediaUrl(filename) : filename) : null;

  const pickImage = async () => {
    if (!tauri) return;
    const path = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
    });
    if (typeof path !== "string") return;
    const saved = await ipc.saveMediaFromPath(path);
    onImageHtmlChange(`<img src="${saved}">`);
  };

  const toNorm = (clientX: number, clientY: number) => {
    const rect = containerRef.current!.getBoundingClientRect();
    return {
      x: (clientX - rect.left) / rect.width,
      y: (clientY - rect.top) / rect.height,
    };
  };

  const onMouseDown = (e: React.MouseEvent) => {
    if (!displaySrc) return;
    const { x, y } = toNorm(e.clientX, e.clientY);
    setDraft({ startX: x, startY: y, x, y });
    setSelected(null);
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (!draft) return;
    const { x, y } = toNorm(e.clientX, e.clientY);
    setDraft((d) => (d ? { ...d, x, y } : d));
  };

  const finishDraft = () => {
    if (!draft) return;
    const left = Math.min(draft.startX, draft.x);
    const top = Math.min(draft.startY, draft.y);
    const width = Math.abs(draft.x - draft.startX);
    const height = Math.abs(draft.y - draft.startY);
    setDraft(null);
    if (width < 0.01 || height < 0.01) return;
    const ord = shapes.length > 0 ? Math.max(...shapes.map((s) => s.ord)) + 1 : 1;
    const shape: OcclusionShape = { ord, kind: "rect", left, top, width, height };
    onOcclusionHtmlChange(serializeOcclusionField([...shapes, shape]));
  };

  const deleteShape = (ord: number) => {
    onOcclusionHtmlChange(serializeOcclusionField(shapes.filter((s) => s.ord !== ord)));
    setSelected(null);
  };

  return (
    <div className="space-y-2">
      {!displaySrc ? (
        <Button variant="outline" size="sm" onClick={() => void pickImage()}>
          <ImagePlus className="mr-1.5 size-3.5" />
          Choose image…
        </Button>
      ) : (
        <>
          <div
            ref={containerRef}
            className="relative w-full select-none overflow-hidden rounded-md border border-border"
            onMouseDown={onMouseDown}
            onMouseMove={onMouseMove}
            onMouseUp={finishDraft}
            onMouseLeave={() => setDraft(null)}
          >
            <img src={displaySrc} alt="" className="block w-full" draggable={false} />
            {shapes.map((s) => (
              <div
                key={s.ord}
                onClick={(e) => {
                  e.stopPropagation();
                  setSelected(s.ord);
                }}
                className={cn(
                  "absolute flex cursor-pointer items-start justify-start border-2",
                  selected === s.ord
                    ? "border-destructive bg-destructive/30"
                    : "border-primary bg-primary/30",
                )}
                style={{
                  left: `${s.left * 100}%`,
                  top: `${s.top * 100}%`,
                  width: `${s.width * 100}%`,
                  height: `${s.height * 100}%`,
                }}
              >
                <span className="bg-primary px-1 text-[10px] leading-4 text-primary-foreground">
                  {s.ord}
                </span>
              </div>
            ))}
            {draft && (
              <div
                className="absolute border-2 border-dashed border-primary bg-primary/20"
                style={{
                  left: `${Math.min(draft.startX, draft.x) * 100}%`,
                  top: `${Math.min(draft.startY, draft.y) * 100}%`,
                  width: `${Math.abs(draft.x - draft.startX) * 100}%`,
                  height: `${Math.abs(draft.y - draft.startY) * 100}%`,
                }}
              />
            )}
          </div>
          <div className="flex items-center justify-between">
            <p className="text-xs text-muted-foreground">
              Drag to mark a region — each becomes its own card. Click a region to select it.
            </p>
            {selected !== null && (
              <Button variant="outline" size="sm" onClick={() => deleteShape(selected)}>
                <Trash2 className="mr-1.5 size-3.5" />
                Delete #{selected}
              </Button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
