// The admin's uploaded logo when there is one, else the built-in brand logo.
import builtIn from "../assets/kk-logo.svg";

export function logoUrl(version?: string | null): string {
  return version ? `api/logo?v=${encodeURIComponent(version)}` : builtIn;
}
