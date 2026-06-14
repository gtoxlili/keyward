import { useState } from "react";
import { IconCheck, IconCopy } from "./icons";
import { useI18n } from "../i18n";
import * as s from "../styles/ui.css";

export function CopyButton({ value, label }: { value: string; label?: string }) {
  const { d } = useI18n();
  const [done, setDone] = useState(false);
  return (
    <button
      className={s.iconbtn}
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
