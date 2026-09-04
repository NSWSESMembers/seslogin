/**
 * @generated SignedSource<<f30a4d1c5cf14f83609aa9325e2db627>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenNewQuery$variables = Record<PropertyKey, never>;
export type ApiTokenNewQuery$data = {
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
};
export type ApiTokenNewQuery = {
  response: ApiTokenNewQuery$data;
  variables: ApiTokenNewQuery$variables;
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
    "name": "ApiTokenNewQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "ApiTokenNewQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "24802ceba1438831732664534dafe412",
    "id": null,
    "metadata": {},
    "name": "ApiTokenNewQuery",
    "operationKind": "query",
    "text": "query ApiTokenNewQuery {\n  locations {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "8284d1c11e8980b2fdf290c35851bc61";

export default node;
