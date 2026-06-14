import type { StatusTone } from "@/components/admin/StatusIndicator";

export function getStatusTone(status: string | undefined): StatusTone {
  const normalized = status?.toLowerCase();

  if (!normalized) return "neutral";
  if (["healthy", "enabled", "active", "ok", "connected", "online"].includes(normalized)) {
    return "success";
  }
  if (["warning", "degraded", "partial", "pending"].includes(normalized)) {
    return "warning";
  }
  if (["unhealthy", "disabled", "error", "failed", "offline", "disconnected"].includes(normalized)) {
    return "danger";
  }

  return "neutral";
}
