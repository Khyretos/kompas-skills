export function relTime(iso: string): string {
  const diff = (Date.now() - new Date(iso).getTime()) / 60_000;
  if (diff < 0) {
    const d = new Date(iso);
    return `at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }
  if (diff < 1) return "just now";
  if (diff < 60) return `${Math.round(diff)} min ago`;
  if (diff < 60 * 24) return `${Math.round(diff / 60)} h ago`;
  return `${Math.round(diff / 60 / 24)} d ago`;
}

export function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
