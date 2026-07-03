import { useRef, useState } from "react";
import type { Editor } from "@tiptap/react";
import katex from "katex";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Bold,
  Italic,
  Underline as UnderlineIcon,
  Strikethrough,
  Subscript as SubscriptIcon,
  Superscript as SuperscriptIcon,
  List,
  ListOrdered,
  Eraser,
  Palette,
  Highlighter,
  Brackets,
  ImagePlus,
  Music,
  Sigma,
} from "lucide-react";
import { ipc, isTauri } from "@/lib/ipc";
import { mediaUrl } from "@/lib/renderCard";
import { cn } from "@/lib/utils";

function ToolbarButton({
  active,
  disabled,
  title,
  onClick,
  children,
}: {
  active?: boolean;
  disabled?: boolean;
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex size-7 items-center justify-center rounded transition-colors disabled:opacity-40",
        active
          ? "bg-primary/15 text-primary"
          : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      )}
    >
      {children}
    </button>
  );
}

/**
 * Formatting toolbar for `FieldEditor`. Operates directly on the TipTap
 * `Editor` instance passed in — no controlled state of its own beyond the
 * two transient popovers (color pickers, math dialog).
 */
export function EditorToolbar({
  editor,
  nextClozeIndex,
  onCloze,
}: {
  editor: Editor;
  /** The cloze index the toolbar button proposes (shown in its tooltip). */
  nextClozeIndex: number;
  onCloze: (index: number) => void;
}) {
  const tauri = isTauri();
  const [mathOpen, setMathOpen] = useState(false);
  const [mathSrc, setMathSrc] = useState("");
  const [mathDisplay, setMathDisplay] = useState(false);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const audioInputRef = useRef<HTMLInputElement>(null);

  const insertImageSrc = (filename: string) => {
    const src = tauri ? mediaUrl(filename) : filename;
    editor.chain().focus().setImage({ src, alt: filename }).run();
  };

  const insertAudioTag = (filename: string) => {
    editor.chain().focus().insertContent(`[sound:${filename}]`).run();
  };

  const pickImage = async () => {
    if (tauri) {
      const path = await open({
        multiple: false,
        filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }],
      });
      if (typeof path !== "string") return;
      const filename = await ipc.saveMediaFromPath(path);
      insertImageSrc(filename);
    } else {
      imageInputRef.current?.click();
    }
  };

  const pickAudio = async () => {
    if (tauri) {
      const path = await open({
        multiple: false,
        filters: [{ name: "Audio", extensions: ["mp3", "ogg", "wav", "m4a"] }],
      });
      if (typeof path !== "string") return;
      const filename = await ipc.saveMediaFromPath(path);
      insertAudioTag(filename);
    } else {
      audioInputRef.current?.click();
    }
  };

  const handleFileInput = async (
    e: React.ChangeEvent<HTMLInputElement>,
    onSaved: (filename: string) => void,
  ) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    if (tauri) {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const filename = await ipc.saveMedia(bytes, file.name);
      onSaved(filename);
    } else {
      onSaved(URL.createObjectURL(file));
    }
  };

  const insertMath = () => {
    const wrapped = mathDisplay ? `\\[${mathSrc}\\]` : `\\(${mathSrc}\\)`;
    editor.chain().focus().insertContent(wrapped).run();
    setMathOpen(false);
    setMathSrc("");
  };

  let mathPreview = "";
  let mathError = false;
  try {
    mathPreview = mathSrc
      ? katex.renderToString(mathSrc, { displayMode: mathDisplay, throwOnError: true })
      : "";
  } catch {
    mathError = true;
  }

  return (
    <div className="flex flex-wrap items-center gap-0.5 border-b border-border bg-muted/40 px-1.5 py-1">
      <ToolbarButton
        title="Bold"
        active={editor.isActive("bold")}
        onClick={() => editor.chain().focus().toggleBold().run()}
      >
        <Bold className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Italic"
        active={editor.isActive("italic")}
        onClick={() => editor.chain().focus().toggleItalic().run()}
      >
        <Italic className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Underline"
        active={editor.isActive("underline")}
        onClick={() => editor.chain().focus().toggleUnderline().run()}
      >
        <UnderlineIcon className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Strikethrough"
        active={editor.isActive("strike")}
        onClick={() => editor.chain().focus().toggleStrike().run()}
      >
        <Strikethrough className="size-3.5" />
      </ToolbarButton>

      <div className="mx-1 h-4 w-px bg-border" />

      <label
        title="Text color"
        className="relative flex size-7 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-accent-foreground"
      >
        <Palette className="size-3.5" />
        <input
          type="color"
          className="absolute inset-0 size-full cursor-pointer opacity-0"
          onChange={(e) => editor.chain().focus().setColor(e.target.value).run()}
        />
      </label>
      <label
        title="Highlight"
        className="relative flex size-7 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-accent-foreground"
      >
        <Highlighter className="size-3.5" />
        <input
          type="color"
          className="absolute inset-0 size-full cursor-pointer opacity-0"
          onChange={(e) => editor.chain().focus().toggleHighlight({ color: e.target.value }).run()}
        />
      </label>
      <ToolbarButton
        title="Subscript"
        active={editor.isActive("subscript")}
        onClick={() => editor.chain().focus().toggleSubscript().run()}
      >
        <SubscriptIcon className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Superscript"
        active={editor.isActive("superscript")}
        onClick={() => editor.chain().focus().toggleSuperscript().run()}
      >
        <SuperscriptIcon className="size-3.5" />
      </ToolbarButton>

      <div className="mx-1 h-4 w-px bg-border" />

      <ToolbarButton
        title="Bullet list"
        active={editor.isActive("bulletList")}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
      >
        <List className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Numbered list"
        active={editor.isActive("orderedList")}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
      >
        <ListOrdered className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title="Clear formatting"
        onClick={() => editor.chain().focus().unsetAllMarks().clearNodes().run()}
      >
        <Eraser className="size-3.5" />
      </ToolbarButton>

      <div className="mx-1 h-4 w-px bg-border" />

      <ToolbarButton title={`Cloze (c${nextClozeIndex})`} onClick={() => onCloze(nextClozeIndex)}>
        <Brackets className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton title="Insert image" onClick={() => void pickImage()}>
        <ImagePlus className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton title="Insert audio" onClick={() => void pickAudio()}>
        <Music className="size-3.5" />
      </ToolbarButton>
      <ToolbarButton title="Insert math" onClick={() => setMathOpen((v) => !v)}>
        <Sigma className="size-3.5" />
      </ToolbarButton>

      {/* Web fallback file inputs (no Tauri dialog available). */}
      <input
        ref={imageInputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => void handleFileInput(e, insertImageSrc)}
      />
      <input
        ref={audioInputRef}
        type="file"
        accept="audio/*"
        className="hidden"
        onChange={(e) => void handleFileInput(e, insertAudioTag)}
      />

      {mathOpen && (
        <div className="w-full space-y-2 border-t border-border p-2">
          <div className="flex items-center gap-2">
            <input
              autoFocus
              value={mathSrc}
              onChange={(e) => setMathSrc(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && mathSrc && !mathError) insertMath();
                if (e.key === "Escape") setMathOpen(false);
              }}
              placeholder="e.g. x^2 + y^2 = z^2"
              className="h-8 flex-1 rounded border border-input bg-background px-2 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
            />
            <label className="flex items-center gap-1 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={mathDisplay}
                onChange={(e) => setMathDisplay(e.target.checked)}
              />
              display
            </label>
            <button
              type="button"
              onClick={insertMath}
              disabled={!mathSrc || mathError}
              className="h-8 rounded bg-primary px-3 text-xs font-medium text-primary-foreground disabled:opacity-40"
            >
              Insert
            </button>
          </div>
          <div
            className={cn(
              "min-h-8 rounded border border-dashed border-border px-2 py-1.5 text-sm",
              mathError && "text-destructive",
            )}
          >
            {mathError ? (
              "Invalid LaTeX"
            ) : mathSrc ? (
              <span dangerouslySetInnerHTML={{ __html: mathPreview }} />
            ) : (
              <span className="text-muted-foreground">Preview</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
