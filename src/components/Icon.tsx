import {
  BellRing,
  CircleAlert,
  CircleCheck,
  CircleSlash,
  CircleX,
  Clock,
  Info,
  Mic,
  ScanLine,
  type LucideIcon,
} from "lucide-react";

/**
 * Único conjunto de iconos de Nexium: Lucide, de trazo, con un grosor fijo y dos tamaños.
 * Ningún otro archivo importa `lucide-react` (lo comprueba scripts/check-tailwind.mjs).
 */
const icons = {
  mic: Mic,
  screen: ScanLine,
  completed: CircleCheck,
  failed: CircleX,
  pending: Clock,
  overdue: CircleAlert,
  cancelled: CircleSlash,
  info: Info,
  reminder: BellRing,
} satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof icons;

/** 16 y 20 px con el texto al 100 %. En rem, para que crezcan con el tamaño de texto del sistema. */
const sizes = {
  sm: { px: 16, className: "size-4" },
  md: { px: 20, className: "size-5" },
} as const;

/** Grosor del trazo con el icono a su tamaño base. `absoluteStrokeWidth` lo iguala entre tamaños. */
const STROKE = 1.75;

interface IconProps {
  name: IconName;
  size?: keyof typeof sizes;
  /** Solo si el icono es la única forma de transmitir algo. Casi siempre va junto a un texto y se oculta. */
  label?: string;
}

export function Icon({ name, size = "sm", label }: IconProps) {
  const Svg = icons[name];
  const { px, className } = sizes[size];
  return (
    <Svg
      size={px}
      strokeWidth={STROKE}
      absoluteStrokeWidth
      className={`shrink-0 ${className}`}
      focusable="false"
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    />
  );
}
