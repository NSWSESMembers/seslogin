/**
 * @generated SignedSource<<793685881ed973fadbca83c2944d51d1>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
export type AuditAction = "CREATE" | "DELETE" | "RESTORE" | "UPDATE" | "%future added value";
export type AuditActorKind = "API_TOKEN" | "PERIOD_LINK" | "SESSION" | "SYSTEM" | "UNAUTHENTICATED" | "UNKNOWN" | "USER" | "%future added value";
export type AuditEntityType = "API_TOKEN" | "CATEGORY" | "LOCATION" | "NITC_GROUP" | "NITC_TAG" | "OAUTH_GRANT" | "PERIOD" | "PERSON" | "SESSION" | "USER" | "USER_TOKEN" | "WEBAUTHN_CREDENTIAL" | "%future added value";
import { FragmentRefs, Result } from "relay-runtime";
export type AuditLog_entry$data = {
  readonly action: AuditAction;
  readonly actor: {
    readonly actorId: string | null | undefined;
    readonly kind: AuditActorKind;
    readonly label: Result<string | null | undefined, unknown>;
    readonly via: string | null | undefined;
  };
  readonly changes: ReadonlyArray<{
    readonly after: string | null | undefined;
    readonly before: string | null | undefined;
    readonly field: string;
  }>;
  readonly entityId: string;
  readonly entityLabel: string | null | undefined;
  readonly entityType: AuditEntityType;
  readonly id: string;
  readonly ip: string | null | undefined;
  readonly location: Result<{
    readonly id: string;
    readonly name: string;
  } | null | undefined, unknown>;
  readonly timestamp: number;
  readonly " $fragmentType": "AuditLog_entry";
};
export type AuditLog_entry$key = {
  readonly " $data"?: AuditLog_entry$data;
  readonly " $fragmentSpreads": FragmentRefs<"AuditLog_entry">;
};

const node: ReaderInlineDataFragment = {
  "kind": "InlineDataFragment",
  "name": "AuditLog_entry"
};

(node as any).hash = "4a6aaf68f4558bf3a7e47da8a52367a5";

export default node;
