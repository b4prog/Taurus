export interface TauriApiError {
  code?: string;
  message?: string;
}

function hasStringProp(value: unknown, key: string): value is Record<string, string> {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as Record<string, unknown>)[key] === "string"
  );
}

export function extractTauriErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (hasStringProp(error, "message")) {
    return error["message"];
  }

  return "An unexpected error occurred while talking to the backend.";
}
