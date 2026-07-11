/**
 * @generated SignedSource<<4f2ecbc868f41c5b47a2e4ab2b342710>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderInlineDataFragment } from 'relay-runtime';
export type NitcExportStatus = "PENDING" | "SYNCED" | "%future added value";
import { FragmentRefs } from "relay-runtime";
export type ActivityListTable_period$data = {
  readonly category: {
    readonly id: string;
    readonly name: string;
  } | null | undefined;
  readonly endTime: number | null | undefined;
  readonly id: string;
  readonly nitcEventId: string | null | undefined;
  readonly nitcExportStatus: NitcExportStatus | null | undefined;
  readonly personId: string | null | undefined;
  readonly signedInSession: {
    readonly id: string;
    readonly name: string;
  } | null | undefined;
  readonly signedOutSession: {
    readonly id: string;
    readonly name: string;
  } | null | undefined;
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

(node as any).hash = "b79b672645ea43f1e35417abedd99311";

export default node;
