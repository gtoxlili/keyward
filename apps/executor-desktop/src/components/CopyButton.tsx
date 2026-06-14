import { useState } from "react";
import { IconCopy, IconCheck } from "./icons";
import { useI18n } from "../i18n";

export function CopyButton({ value, label }: { value: string; label?: string }) {
  const { d } = useI18n();
  const [done, setDone] = useState(false);
  return (
    <button
      className="iconbtn"
      title={d.common.copy}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(value);
          setDone(true);
          setTimeout(() => setDone(false), 1200);
        } catch {
          /* ignore */
        }
      }}
    >
      {done ? <IconCheck size={15} /> : <IconCopy size={15} />}
      {label && <span>{done ? d.common.copied : label}</span>}
    </button>
  );
}
