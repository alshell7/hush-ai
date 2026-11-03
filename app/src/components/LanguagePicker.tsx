// A language from Live Translate's list, by name in the user's own language.
import { Label, ListBox, Select } from "@heroui/react";
import { useEffect, useMemo, useState } from "react";
import { features, languageName } from "../lib/features";

/** Widely spoken languages first, then the rest by name. */
const FIRST = ["en", "es", "fr", "de", "pt-BR", "hi", "ar", "zh-Hans", "ja", "ko", "it", "ru", "tr", "id"];

let cached: string[] | null = null;

export function useLanguages() {
  const [codes, setCodes] = useState<string[]>(cached ?? []);
  useEffect(() => {
    if (cached) return;
    features.translateLanguages().then((list) => {
      cached = list;
      setCodes(list);
    });
  }, []);
  return useMemo(() => {
    const first = FIRST.filter((c) => codes.includes(c));
    const rest = codes.filter((c) => !first.includes(c)).sort((a, b) => languageName(a).localeCompare(languageName(b)));
    return [...first, ...rest];
  }, [codes]);
}

export function LanguagePicker({ label, value, onChange, compact = false }: { label: string; value: string; onChange: (code: string) => void; compact?: boolean }) {
  const languages = useLanguages();
  const items = languages.includes(value) ? languages : [value, ...languages];
  return (
    <Select className="w-full min-w-0" aria-label={label} value={value} onChange={(next) => next && onChange(String(next))}>
      <Label className="sr-only">{label}</Label>
      <Select.Trigger className={`${compact ? "h-9" : "h-11"} rounded-2xl bg-white/80 pl-3`}>
        <Select.Value className={`truncate ${compact ? "text-[12.5px]" : "text-[13.5px]"} font-[560]`} />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover className="max-h-[320px] rounded-2xl">
        <ListBox>
          {items.map((code) => (
            <ListBox.Item key={code} id={code} textValue={languageName(code)}>
              {languageName(code)}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
