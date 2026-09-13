import { useId, useRef, useState } from "react";
import { Check, ChevronDown, ChevronUp, Plus } from "lucide-react";
import type { Category } from "./types";
import { categoryColor } from "./charts";

const PALETTE = [
  "#b77340",
  "#90b341",
  "#40aab5",
  "#6b83c7",
  "#9270bc",
  "#bb6e92",
  "#bb645c",
  "#b79843",
  "#579a79",
  "#728c9a",
  "#a47b60",
  "#7079b5",
];

export function CategoryPicker({
  categories,
  colors,
  value,
  onChange,
  saveColor,
  disabled,
  allowNew = false,
  label = "Category",
}: {
  categories: Category[];
  colors: Record<string, string>;
  value: string;
  onChange: (id: string) => void;
  saveColor: (id: string, color: string) => Promise<boolean>;
  disabled?: boolean;
  allowNew?: boolean;
  label?: string;
}) {
  const [open, setOpen] = useState(false),
    [query, setQuery] = useState("");
  const [editingColor, setEditingColor] = useState(false),
    [hex, setHex] = useState("");
  const id = useId(),
    trigger = useRef<HTMLButtonElement>(null);
  const selected = categories.find((c) => c.id === value);
  const choices = categories.filter((c) =>
    c.name.toLowerCase().includes(query.toLowerCase()),
  );
  const close = () => {
    setOpen(false);
    setEditingColor(false);
    setQuery("");
    trigger.current?.focus();
  };
  const choose = (id: string) => {
    onChange(id);
    close();
  };
  return (
    <div
      className="category-picker"
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) {
          e.preventDefault();
          e.stopPropagation();
          close();
        }
      }}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) {
          setOpen(false);
          setEditingColor(false);
        }
      }}
    >
      <button
        type="button"
        ref={trigger}
        className="category-trigger"
        aria-label={label}
        aria-expanded={open}
        aria-controls={id}
        disabled={disabled}
        onClick={() => {
          setOpen(!open);
          setEditingColor(false);
        }}
      >
        <span>
          {selected && (
            <i
              className="color-dot"
              style={{ background: categoryColor(value, colors) }}
            />
          )}
          {selected?.name || (value === "new" ? "New category" : "Category")}
        </span>
        <ChevronDown size={16} className={open ? "is-open" : ""} />
      </button>
      {open && (
        <div className="category-menu" id={id}>
          <input
            aria-label="Search categories"
            placeholder="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div
            className="category-options"
            role="listbox"
            aria-label="Categories"
          >
            {choices.map((c) => (
              <button
                type="button"
                role="option"
                aria-selected={c.id === value}
                key={c.id}
                onClick={() => choose(c.id)}
              >
                <i
                  className="color-dot"
                  style={{ background: categoryColor(c.id, colors) }}
                />
                <span>{c.name}</span>
                {c.id === value && <Check size={15} />}
              </button>
            ))}
            {!choices.length && (
              <span className="no-categories">No categories</span>
            )}
          </div>
          {allowNew && (
            <button
              type="button"
              className="new-category-option"
              onClick={() => choose("new")}
            >
              <Plus size={15} />
              New category
            </button>
          )}
          {selected && (
            <>
              <button
                type="button"
                className="color-edit-toggle"
                onClick={() => {
                  setEditingColor(!editingColor);
                  setHex(colors[value] || "");
                }}
              >
                Edit color{" "}
                <i
                  className="color-dot"
                  style={{ background: categoryColor(value, colors) }}
                />
              </button>
              {editingColor && (
                <div className="color-editor">
                  <div className="color-palette" aria-label="Category colors">
                    {PALETTE.map((color) => (
                      <button
                        type="button"
                        key={color}
                        disabled={disabled}
                        aria-label={`Use color ${color}`}
                        aria-pressed={colors[value] === color}
                        style={{ background: color }}
                        onClick={async () => {
                          if (await saveColor(value, color))
                            setEditingColor(false);
                        }}
                      />
                    ))}
                  </div>
                  <div className="custom-color">
                    <input
                      aria-label="Hex color"
                      value={hex}
                      maxLength={7}
                      placeholder="#RRGGBB"
                      onChange={(e) => setHex(e.target.value)}
                    />
                    <button
                      type="button"
                      disabled={disabled || !/^#[0-9a-f]{6}$/i.test(hex)}
                      onClick={async () => {
                        if (await saveColor(value, hex)) setEditingColor(false);
                      }}
                    >
                      Apply
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

export function ClockInput({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  const parts = value.split(":");
  const change = (i: number, text: string) =>
    onChange(parts.map((part, n) => (n === i ? text : part)).join(":"));
  const step = (i: number, delta: number) => {
    const limit = i === 0 ? 24 : 60;
    change(
      i,
      String(((Number(parts[i]) || 0) + delta + limit) % limit).padStart(
        2,
        "0",
      ),
    );
  };
  return (
    <fieldset className="clock-input" disabled={disabled}>
      <legend>{label}</legend>
      <div className="clock-digits">
        {parts.map((part, i) => (
          <div className="clock-part" key={i}>
            <button
              type="button"
              aria-label={`Increase ${label.toLowerCase()} ${i ? "minutes" : "hours"}`}
              onClick={() => step(i, 1)}
            >
              <ChevronUp size={16} />
            </button>
            <input
              type="text"
              inputMode="numeric"
              autoComplete="off"
              aria-label={`${label} ${i ? "minutes" : "hours"}`}
              maxLength={2}
              value={part}
              onFocus={(e) => e.currentTarget.select()}
              onChange={(e) => {
                if (/^\d{0,2}$/.test(e.target.value)) change(i, e.target.value);
              }}
              onBlur={() => {
                if (part) change(i, part.padStart(2, "0"));
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowUp" || e.key === "ArrowDown") {
                  e.preventDefault();
                  step(i, e.key === "ArrowUp" ? 1 : -1);
                }
              }}
            />
            <button
              type="button"
              aria-label={`Decrease ${label.toLowerCase()} ${i ? "minutes" : "hours"}`}
              onClick={() => step(i, -1)}
            >
              <ChevronDown size={16} />
            </button>
            {i === 0 && (
              <span className="clock-colon" aria-hidden="true">
                :
              </span>
            )}
          </div>
        ))}
      </div>
    </fieldset>
  );
}
