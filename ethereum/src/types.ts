export interface DecodedResult<T = unknown> {
  args: Array<{ data: T }>;
}

export enum STATUS {
  INITIATED = "initiated",
  IN_PROGRESS = "in_progress",
  CLAIM_READY = "claim_ready",
  BRIDGED = "bridged",
}
