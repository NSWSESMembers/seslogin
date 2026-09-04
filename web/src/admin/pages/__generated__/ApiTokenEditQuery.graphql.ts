/**
 * @generated SignedSource<<e1d032cfc3383c0c38cf9d2aaf0851c3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenEditQuery$variables = {
  id: string;
};
export type ApiTokenEditQuery$data = {
  readonly apiToken: {
    readonly createdAt: number;
    readonly expiresAt: number | null | undefined;
    readonly id: string;
    readonly lastUsedAt: number | null | undefined;
    readonly locationGrants: ReadonlyArray<string>;
    readonly name: string;
    readonly readOnly: boolean;
    readonly revokedAt: number | null | undefined;
  };
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
};
export type ApiTokenEditQuery = {
  response: ApiTokenEditQuery$data;
  variables: ApiTokenEditQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v3 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "concreteType": "ApiToken",
    "kind": "LinkedField",
    "name": "apiToken",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      (v2/*: any*/),
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
        "name": "locationGrants",
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
        "name": "createdAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastUsedAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "revokedAt",
        "storageKey": null
      }
    ],
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
      (v1/*: any*/),
      (v2/*: any*/)
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ApiTokenEditQuery",
    "selections": (v3/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ApiTokenEditQuery",
    "selections": (v3/*: any*/)
  },
  "params": {
    "cacheID": "e7858a29249353d066298b6213f13228",
    "id": null,
    "metadata": {},
    "name": "ApiTokenEditQuery",
    "operationKind": "query",
    "text": "query ApiTokenEditQuery(\n  $id: ID!\n) {\n  apiToken(id: $id) {\n    id\n    name\n    readOnly\n    locationGrants\n    expiresAt\n    createdAt\n    lastUsedAt\n    revokedAt\n  }\n  locations {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "66ed01e3194bc0bd5719930ebf0779ab";

export default node;
