import type { ComponentPropsWithRef } from "react";

export type ButtonVariant = "primary" | "secondary" | "danger" | "signal" | "onSignal";

const variants: Record<ButtonVariant, string> = {
  primary: "bg-ink text-panel border-ink font-semibold",
  secondary: "bg-transparent text-ink border-line hover:border-ink",
  danger: "bg-transparent text-error border-error",
  signal: "bg-on-signal text-signal border-on-signal font-semibold",
  onSignal: "bg-transparent text-on-signal border-on-signal",
};

const disabledStyle = "disabled:cursor-not-allowed disabled:bg-transparent disabled:text-muted disabled:border-line disabled:border-dashed disabled:font-normal";

interface ButtonProps extends ComponentPropsWithRef<"button"> {
  variant?: ButtonVariant;
}

/** Alto mínimo de 44 px y texto que puede partirse en dos líneas antes que salirse del botón. */
export function Button({ variant = "secondary", className = "", type = "button", ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex min-h-11 max-w-full cursor-pointer items-center justify-center gap-2 rounded-sm border px-4 py-1 text-center ${variants[variant]} ${disabledStyle} ${className}`}
      {...props}
    />
  );
}
