/**
 * @generated SignedSource<<8aabd7d051930594bcf238240ec7a29a>>
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
    readonly isVirtual: boolean;
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

(node as any).hash = "fae39eed8e70c88d590e40eea41a6b01";

export default node;
