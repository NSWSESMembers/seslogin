/**
 * @generated SignedSource<<fccbe99c54fa27c9fee42ecc04ad7450>>
 * @lightSyntaxTransform
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
    "metadata": {
      "throwOnFieldError": true
    },
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
          (v0/*:: as any*/)
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
          (v0/*:: as any*/),
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

(node as any).hash = "0593ce15ff4aa50c796ddb7049286267";

export default node;
