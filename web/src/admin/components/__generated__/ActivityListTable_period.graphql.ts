/**
 * @generated SignedSource<<c0b54d6bb9173ed39d922ac8efd47fd6>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
export type NitcExportStatus = "PENDING" | "SYNCED" | "%future added value";
import { FragmentRefs, Result } from "relay-runtime";
export type ActivityListTable_period$data = {
  readonly category: Result<{
    readonly id: string;
    readonly isVirtual: boolean;
    readonly name: string;
  } | null | undefined, unknown>;
  readonly comment: string | null | undefined;
  readonly endTime: number | null | undefined;
  readonly id: string;
  readonly nitcEventId: string | null | undefined;
  readonly nitcExportStatus: NitcExportStatus | null | undefined;
  readonly personId: string | null | undefined;
  readonly signedInSession: Result<{
    readonly id: string;
    readonly name: string;
  } | null | undefined, unknown>;
  readonly signedOutSession: Result<{
    readonly id: string;
    readonly name: string;
  } | null | undefined, unknown>;
  readonly startTime: number;
  readonly " $fragmentType": "ActivityListTable_period";
};
export type ActivityListTable_period$key = {
  readonly " $data"?: ActivityListTable_period$data;
  readonly " $fragmentSpreads": FragmentRefs<"ActivityListTable_period">;
};

const node: ReaderInlineDataFragment = {
  "kind": "InlineDataFragment",
  "name": "ActivityListTable_period"
};

(node as any).hash = "ee29059716c9eead19cf8ca517362bfe";

export default node;
