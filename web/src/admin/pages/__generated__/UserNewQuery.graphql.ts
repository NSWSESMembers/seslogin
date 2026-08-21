/**
 * @generated SignedSource<<b6388a2f2c72d8be43f825ad35403ed1>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserNewQuery$variables = Record<PropertyKey, never>;
export type UserNewQuery$data = {
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
};
export type UserNewQuery = {
  response: UserNewQuery$data;
  variables: UserNewQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "locations",
    "plural": true,
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
        "name": "name",
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
    "name": "UserNewQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "UserNewQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "6d6b5d994f963862139042031bc09a43",
    "id": null,
    "metadata": {},
    "name": "UserNewQuery",
    "operationKind": "query",
    "text": "query UserNewQuery {\n  locations {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "3a1a7cc7ccecb7ff5f7f7dc63093b1d1";

export default node;
