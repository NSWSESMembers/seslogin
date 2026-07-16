/**
 * @generated SignedSource<<28c091c2f5c074d645e1f757a10eb5a3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsActivityDisplayQuery$variables = Record<PropertyKey, never>;
export type SettingsActivityDisplayQuery$data = {
  readonly user: {
    readonly disaggregateVirtualPeriods: boolean;
  };
};
export type SettingsActivityDisplayQuery = {
  response: SettingsActivityDisplayQuery$data;
  variables: SettingsActivityDisplayQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "disaggregateVirtualPeriods",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsActivityDisplayQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v0/*: any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsActivityDisplayQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v0/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "dc50e1ab84664a008658ed9254a0e71a",
    "id": null,
    "metadata": {},
    "name": "SettingsActivityDisplayQuery",
    "operationKind": "query",
    "text": "query SettingsActivityDisplayQuery {\n  user {\n    disaggregateVirtualPeriods\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "2078030de1caa865b137cb11261f5e8d";

export default node;
