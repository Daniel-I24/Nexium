import { Icon, type IconName } from "./Icon";

export type Status = "completed" | "failed" | "pending" | "overdue" | "cancelled";

const styles: Record<Status, { icon: IconName; color: string }> = {
  completed: { icon: "completed", color: "text-ok" },
  failed: { icon: "failed", color: "text-error" },
  pending: { icon: "pending", color: "text-query" },
  overdue: { icon: "overdue", color: "text-error" },
  cancelled: { icon: "cancelled", color: "text-muted" },
};

/** Un estado nunca se distingue solo por el color: siempre lleva icono y palabra. */
export function StatusLabel({ status, children }: { status: Status; children: string }) {
  const { icon, color } = styles[status];
  return (
    <span data-status={status} className={`inline-flex items-center gap-1 font-medium ${color}`}>
      <Icon name={icon} />
      {children}
    </span>
  );
}
