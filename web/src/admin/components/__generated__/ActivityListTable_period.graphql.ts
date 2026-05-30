/**
 * @generated SignedSource<<571a61726a61a53aee316aa2ac9ca79d>>
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

(node as any).hash = "fa0085b8a4dec83b1f691927fb22e047";

export default node;
