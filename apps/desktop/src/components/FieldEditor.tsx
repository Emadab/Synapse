import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { TextStyle } from "@tiptap/extension-text-style";
import Color from "@tiptap/extension-color";
import Highlight from "@tiptap/extension-highlight";
import Subscript from "@tiptap/extension-subscript";
import Superscript from "@tiptap/extension-superscript";
import Image from "@tiptap/extension-image";
import { EditorToolbar } from "@/components/editor/EditorToolbar";
import { isTauri } from "@/lib/ipc";
import { toDisplayHtml, toStorageHtml } from "@/lib/editorMedia";
import { maxClozeIndex } from "@/lib/clozeUtils";

/**
 * Rich-text editor for a single note field. Edits HTML in place; `onChange`
 * fires with the field's current HTML. Remount (via a React key) to load a
 * different field's content — the initial value is only read on mount.
 */
export function FieldEditor({
  value,
  onChange,
  otherFieldsHtml,
}: {
  value: string;
  onChange: (html: string) => void;
  /** HTML of the note's other fields, so cloze numbering stays unique across the whole note. */
  otherFieldsHtml?: string[];
}) {
  const tauri = isTauri();

  // Recomputed on every render (cheap regex scans) so the toolbar's cloze
  // button always proposes the next free index across the whole note.
  const nextCloze = maxClozeIndex([...(otherFieldsHtml ?? []), value]) + 1;
  const lastCloze = Math.max(1, nextCloze - 1);

  const editor = useEditor({
    extensions: [
      StarterKit,
      TextStyle,
      Color,
      Highlight.configure({ multicolor: true }),
      Subscript,
      Superscript,
      Image,
    ],
    content: toDisplayHtml(value, tauri),
    immediatelyRender: false,
    editorProps: {
      attributes: { class: "synapse-prose min-h-[3.5rem] px-3 py-2 outline-none" },
      handleKeyDown: (_view, event) => {
        if (!(event.ctrlKey && event.shiftKey && event.key.toLowerCase() === "c")) return false;
        event.preventDefault();
        insertCloze(event.altKey ? lastCloze : nextCloze);
        return true;
      },
    },
    onUpdate: ({ editor }) => onChange(toStorageHtml(editor.getHTML())),
  });

  function insertCloze(index: number) {
    if (!editor) return;
    const { state } = editor;
    const { from, to, empty } = state.selection;
    const text = empty ? "…" : state.doc.textBetween(from, to);
    editor.chain().focus().insertContent(`{{c${index}::${text}}}`).run();
  }

  if (!editor) return null;

  return (
    <div className="overflow-hidden rounded-md border border-input bg-background text-sm focus-within:ring-2 focus-within:ring-ring">
      <EditorToolbar editor={editor} nextClozeIndex={nextCloze} onCloze={insertCloze} />
      <EditorContent editor={editor} />
    </div>
  );
}
