/**
 * @generated SignedSource<<02cff2d9eb3f272d9aa27f2527a081af>>
 * @lightSyntaxTransform
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
  readonly session: {
    readonly config: any;
    readonly id: string;
    readonly keyExpiresAt: number | null | undefined;
    readonly location: {
      readonly id: string;
      readonly name: string;
    };
    readonly name: string;
    readonly version: number;
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
    "alias": null,
    "args": null,
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "session",
    "plural": false,
    "selections": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
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
        "name": "version",
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
          (v0/*:: as any*/),
          (v1/*:: as any*/)
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
    "selections": (v2/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "KioskTokenSessionFetcherQuery",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "b512c18913aba3d9b810a55edad6f1b2",
    "id": null,
    "metadata": {},
    "name": "KioskTokenSessionFetcherQuery",
    "operationKind": "query",
    "text": "query KioskTokenSessionFetcherQuery {\n  session {\n    id\n    name\n    config\n    version\n    keyExpiresAt\n    location {\n      id\n      name\n    }\n  }\n  environment {\n    gitRev\n    isProdDb\n  }\n}\n"
  }
};
})();

(node as any).hash = "21d00cabcf31009539f645074e9780e7";

export default node;
