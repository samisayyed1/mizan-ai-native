import type { ProviderHealth } from "@/lib/types";

export interface CircuitVisual {
  variant: "success" | "warning" | "destructive";
  label: string;
}

/** Map a provider's circuit-breaker state to a badge variant + label.
 * Pure so it can be unit-tested without rendering. */
export function circuitStateVisual(state: ProviderHealth["circuitState"]): CircuitVisual {
  switch (state) {
    case "Open":
      return { variant: "destructive", label: "Failing" };
    case "HalfOpen":
      return { variant: "warning", label: "Recovering" };
    case "Closed":
    default:
      return { variant: "success", label: "Operational" };
  }
}
