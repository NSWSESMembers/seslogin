/**
 * @generated SignedSource<<d0c7a41efb51b27786609515cff0bcde>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type ApiTokenListQuery$variables = Record<PropertyKey, never>;
export type ApiTokenListQuery$data = {
  readonly apiTokens: ReadonlyArray<{
    readonly id: string;
    readonly " $fragmentSpreads": FragmentRefs<"ApiTokenList_token">;
  }>;
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
};
export type ApiTokenListQuery = {
  response: ApiTokenListQuery$data;
  variables: ApiTokenListQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "concreteType": "Location",
  "kind": "LinkedField",
  "name": "locations",
  "plural": true,
  "selections": [
    (v0/*: any*/),
    (v1/*: any*/)
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ApiTokenListQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "ApiToken",
        "kind": "LinkedField",
        "name": "apiTokens",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "ApiTokenList_token"
          }
        ],
        "storageKey": null
      },
      (v2/*: any*/)
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "ApiTokenListQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "ApiToken",
        "kind": "LinkedField",
        "name": "apiTokens",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "locationGrants",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "readOnly",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "createdAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "expiresAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastUsedAt",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      (v2/*: any*/)
    ]
  },
  "params": {
    "cacheID": "9fe82fa4e9dc131d1a141e0c98575d50",
    "id": null,
    "metadata": {},
    "name": "ApiTokenListQuery",
    "operationKind": "query",
    "text": "query ApiTokenListQuery {\n  apiTokens {\n    id\n    ...ApiTokenList_token\n  }\n  locations {\n    id\n    name\n  }\n}\n\nfragment ApiTokenList_token on ApiToken {\n  id\n  name\n  locationGrants\n  readOnly\n  createdAt\n  expiresAt\n  lastUsedAt\n}\n"
  }
};
})();

(node as any).hash = "45473079acf5507b269d4e39a5d2e804";

export default node;
