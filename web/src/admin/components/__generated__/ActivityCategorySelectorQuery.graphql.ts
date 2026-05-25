/**
 * @generated SignedSource<<50b33d0da4395a577491931dc8437135>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityCategorySelectorQuery$variables = Record<PropertyKey, never>;
export type ActivityCategorySelectorQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
};
export type ActivityCategorySelectorQuery = {
  response: ActivityCategorySelectorQuery$data;
  variables: ActivityCategorySelectorQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "categories",
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
    "metadata": null,
    "name": "ActivityCategorySelectorQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "ActivityCategorySelectorQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "746a0841caa34322e4a6b7c08ef12759",
    "id": null,
    "metadata": {},
    "name": "ActivityCategorySelectorQuery",
    "operationKind": "query",
    "text": "query ActivityCategorySelectorQuery {\n  categories {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "a11125b852a6294df9a32009b5449b0d";

export default node;
