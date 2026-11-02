/**
 * @generated SignedSource<<a7940b5c93b3111024d099b0457bca6a>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsDailyChangesMutation$variables = {
  dailyChangesLocationIds: ReadonlyArray<string>;
  dailyLocationIds: ReadonlyArray<string>;
};
export type SettingsDailyChangesMutation$data = {
  readonly updateMyEmailConfig: {
    readonly dailyChangesLocationIds: ReadonlyArray<string>;
    readonly id: string;
  };
};
export type SettingsDailyChangesMutation = {
  response: SettingsDailyChangesMutation$data;
  variables: SettingsDailyChangesMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "dailyChangesLocationIds"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "dailyLocationIds"
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "dailyChangesLocationIds",
        "variableName": "dailyChangesLocationIds"
      },
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
        "name": "dailyChangesLocationIds",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsDailyChangesMutation",
    "selections": (v2/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "SettingsDailyChangesMutation",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "6affa5d65fddb805104bd0074da68c20",
    "id": null,
    "metadata": {},
    "name": "SettingsDailyChangesMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsDailyChangesMutation(\n  $dailyLocationIds: [String!]!\n  $dailyChangesLocationIds: [String!]!\n) {\n  updateMyEmailConfig(dailyLocationIds: $dailyLocationIds, dailyChangesLocationIds: $dailyChangesLocationIds) {\n    id\n    dailyChangesLocationIds\n  }\n}\n"
  }
};
})();

(node as any).hash = "b1513633fc0601ba85d1856f6a7caa87";

export default node;
