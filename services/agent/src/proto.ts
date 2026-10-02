export interface Proto {
    approvals?:   SignedApproval[];
    hash_after?:  string;
    hash_before?: string;
    id:           string;
    kind:         Kind;
    paths:        string[];
    timestamp:    Date;
    [property: string]: unknown;
}

export interface SignedApproval {
    id:        string;
    signature: string;
    [property: string]: unknown;
}

export type Kind = "create" | "modify" | "delete";
