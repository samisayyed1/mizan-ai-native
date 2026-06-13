import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** Tailwind-aware class merger; resolves conflicts (e.g. `p-4 p-6` → `p-6`). */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
