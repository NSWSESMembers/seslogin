/**
 * @generated SignedSource<<9d02592a95a1b6668b3f1d20e9307e16>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsDailyEmailQuery$variables = Record<PropertyKey, never>;
export type SettingsDailyEmailQuery$data = {
  readonly user: {
    readonly emailSummaryLocationIds: ReadonlyArray<string>;
    readonly id: string;
    readonly locations: ReadonlyArray<{
      readonly id: string;
      readonly name: string;
    }>;
  };
};
export type SettingsDailyEmailQuery = {
  response: SettingsDailyEmailQuery$data;
  variables: SettingsDailyEmailQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = [
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
        "name": "emailSummaryLocationIds",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "locations",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SettingsDailyEmailQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsDailyEmailQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "ca98825c98b88e318a2162f605f3d0ef",
    "id": null,
    "metadata": {},
    "name": "SettingsDailyEmailQuery",
    "operationKind": "query",
    "text": "query SettingsDailyEmailQuery {\n  user {\n    id\n    emailSummaryLocationIds\n    locations {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "dbbacde83474d5e37c72f296e6066c7d";

export default node;
