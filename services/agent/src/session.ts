export interface PlannerSessionParams {
  request: string;
  publicFacts: unknown[];
  fields: unknown[];
}

/** Builds the sanitised context sent to the online planner. Placeholder. */
export class OnlinePlannerSession {
  static build(params: PlannerSessionParams): string {
    void params;
    return 'Clean session data';
  }
}
