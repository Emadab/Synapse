import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";

/**
 * Rich-text editor for a single note field. Edits HTML in place; `onChange`
 * fires with the field's current HTML. Remount (via a React key) to load a
 * different field's content — the initial value is only read on mount.
 */
export function FieldEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (html: string) => void;
}) {
  const editor = useEditor({
    extensions: [StarterKit],
    content: value,
    immediatelyRender: false,
    editorProps: {
      attributes: { class: "synapse-prose min-h-[3.5rem] px-3 py-2 outline-none" },
    },
    onUpdate: ({ editor }) => onChange(editor.getHTML()),
  });

  return (
    <EditorContent
      editor={editor}
      className="rounded-md border border-input bg-background text-sm focus-within:ring-2 focus-within:ring-ring"
    />
  );
}
