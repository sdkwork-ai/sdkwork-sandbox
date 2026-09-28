export interface ProblemDetail {
  type: string;
  title: string;
  status: number;
  detail?: string;
  /** Request endpoint occurrence: {METHOD} {routeTemplate} */
  instance?: string;
  operationId?: string;
  /** Numeric error result code; MUST be non-zero (API_SPEC.md section 15.3) */
  code: number;
  traceId: string;
}
