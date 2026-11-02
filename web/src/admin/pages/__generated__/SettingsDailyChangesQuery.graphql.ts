/**
 * @generated SignedSource<<1880611b2628a5d7d231d58ecbee4b39>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsDailyChangesQuery$variables = Record<PropertyKey, never>;
export type SettingsDailyChangesQuery$data = {
  readonly user: {
    readonly dailyChangesLocationIds: ReadonlyArray<string>;
    readonly emailSummaryLocationIds: ReadonlyArray<string>;
    readonly id: string;
    readonly locations: ReadonlyArray<{
      readonly id: string;
      readonly name: string;
    }>;
  };
};
export type SettingsDailyChangesQuery = {
  response: SettingsDailyChangesQuery$data;
  variables: SettingsDailyChangesQuery$variables;
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
      (v0/*:: as any*/),
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
        "kind": "ScalarField",
        "name": "dailyChangesLocationIds",
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
          (v0/*:: as any*/),
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
    "name": "SettingsDailyChangesQuery",
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsDailyChangesQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "f676b036da4bf683534069b94d503c17",
    "id": null,
    "metadata": {},
    "name": "SettingsDailyChangesQuery",
    "operationKind": "query",
    "text": "query SettingsDailyChangesQuery {\n  user {\n    id\n    emailSummaryLocationIds\n    dailyChangesLocationIds\n    locations {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "a70b6b3a98c12a7cd0f5429bddc403ac";

export default node;
