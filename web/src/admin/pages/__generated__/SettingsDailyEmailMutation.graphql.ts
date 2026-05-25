/**
 * @generated SignedSource<<8fd8db583fb2a15163d40714d10d9c80>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsDailyEmailMutation$variables = {
  dailyLocationIds: ReadonlyArray<string>;
};
export type SettingsDailyEmailMutation$data = {
  readonly updateMyEmailConfig: {
    readonly emailSummaryLocationIds: ReadonlyArray<string>;
    readonly id: string;
  };
};
export type SettingsDailyEmailMutation = {
  response: SettingsDailyEmailMutation$data;
  variables: SettingsDailyEmailMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "dailyLocationIds"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "dailyLocationIds",
        "variableName": "dailyLocationIds"
      }
    ],
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "updateMyEmailConfig",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "emailSummaryLocationIds",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsDailyEmailMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsDailyEmailMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "d462b0b2a92bdf79f4ae8dc1b127fe0f",
    "id": null,
    "metadata": {},
    "name": "SettingsDailyEmailMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsDailyEmailMutation(\n  $dailyLocationIds: [String!]!\n) {\n  updateMyEmailConfig(dailyLocationIds: $dailyLocationIds) {\n    id\n    emailSummaryLocationIds\n  }\n}\n"
  }
};
})();

(node as any).hash = "101b3c960ec9c355e04d7bee7e49ae6c";

export default node;
