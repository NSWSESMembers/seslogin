/**
 * @generated SignedSource<<b62d3cf8b5b7120a1072b8d27e328049>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskTokenSessionFetcherQuery$variables = Record<PropertyKey, never>;
export type KioskTokenSessionFetcherQuery$data = {
  readonly environment: {
    readonly gitRev: string;
    readonly isProdDb: boolean;
  };
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
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "Environment",
    "kind": "LinkedField",
    "name": "environment",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gitRev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isProdDb",
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
    "cacheID": "37ec4bde83224d824bfd485e575b73ae",
    "id": null,
    "metadata": {},
    "name": "KioskTokenSessionFetcherQuery",
    "operationKind": "query",
    "text": "query KioskTokenSessionFetcherQuery {\n  refresh_token: refreshToken\n  session {\n    id\n    name\n    config\n    keyExpiresAt\n    location {\n      id\n      name\n    }\n  }\n  environment {\n    gitRev\n    isProdDb\n  }\n}\n"
  }
};
})();

(node as any).hash = "7fb6dc55e941f3630dd68a7563d88f52";

export default node;
