import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "danger" | "signal" | "onSignal";

const variants: Record<ButtonVariant, string> = {
  primary: "bg-ink text-panel border-ink font-semibold",
  secondary: "bg-transparent text-ink border-line",
  danger: "bg-transparent text-error border-error",
  signal: "bg-on-signal text-signal border-on-signal font-semibold",
  onSignal: "bg-transparent text-on-signal border-on-signal",
};

const disabledStyle = "disabled:bg-transparent disabled:text-muted disabled:border-line disabled:border-dashed disabled:font-normal";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

export function Button({ variant = "secondary", className = "", type = "button", ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex min-h-11 items-center justify-center gap-2 rounded-sm border px-4 transition-colors duration-150 ease-instrument ${variants[variant]} ${disabledStyle} ${className}`}
      {...props}
    />
  );
}
