/**
 * @generated SignedSource<<c22cac0a57eb827295e7ad7677f50819>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskTokenSessionFetcherQuery$variables = Record<PropertyKey, never>;
export type KioskTokenSessionFetcherQuery$data = {
  readonly refresh_token: string;
  readonly session: {
    readonly config: any;
    readonly id: string;
    readonly keyExpiresAt: number | null | undefined;
    readonly location: {
      readonly id: string;
      readonly name: string;
    };
    readonly name: string;
  };
};
export type KioskTokenSessionFetcherQuery = {
  response: KioskTokenSessionFetcherQuery$data;
  variables: KioskTokenSessionFetcherQuery$variables;
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
v2 = [
  {
    "alias": "refresh_token",
    "args": null,
    "kind": "ScalarField",
    "name": "refreshToken",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "session",
    "plural": false,
    "selections": [
      (v0/*: any*/),
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "config",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "keyExpiresAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/)
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
    "metadata": null,
    "name": "KioskTokenSessionFetcherQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "KioskTokenSessionFetcherQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "27da5ee1cdb1a65b75b75cc697c3e707",
    "id": null,
    "metadata": {},
    "name": "KioskTokenSessionFetcherQuery",
    "operationKind": "query",
    "text": "query KioskTokenSessionFetcherQuery {\n  refresh_token: refreshToken\n  session {\n    id\n    name\n    config\n    keyExpiresAt\n    location {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "71db2dc6dbeb9ffbff9626857ead5804";

export default node;
