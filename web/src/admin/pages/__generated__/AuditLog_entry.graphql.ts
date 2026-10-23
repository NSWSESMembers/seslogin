/**
 * @generated SignedSource<<0f8f406c5c7305f39fe1316b2b6e94f0>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
export type AuditAction = "CREATE" | "DELETE" | "RESTORE" | "UPDATE" | "%future added value";
export type AuditActorKind = "API_TOKEN" | "PERIOD_LINK" | "SESSION" | "SYSTEM" | "UNAUTHENTICATED" | "UNKNOWN" | "USER" | "%future added value";
export type AuditEntityType = "API_TOKEN" | "CATEGORY" | "EQUIPMENT" | "EQUIPMENT_LOG_ENTRY" | "EQUIPMENT_RESERVATION" | "LOCATION" | "MAP_LAYER" | "MAP_PIN" | "NITC_GROUP" | "NITC_TAG" | "NOTE" | "NOTE_FOLDER" | "OAUTH_GRANT" | "ON_CALL_ROLE" | "PERIOD" | "PERSON" | "SESSION" | "TASK" | "TASK_LIST" | "USER" | "USER_TOKEN" | "WEBAUTHN_CREDENTIAL" | "%future added value";
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
