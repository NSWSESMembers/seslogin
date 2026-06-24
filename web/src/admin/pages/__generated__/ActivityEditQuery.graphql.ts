/**
 * @generated SignedSource<<20102eaabf24b52f9aff4c6748631c3b>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityEditQuery$variables = {
  id: string;
};
export type ActivityEditQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
  }>;
  readonly period: {
    readonly category: {
      readonly id: string;
      readonly name: string;
    } | null | undefined;
    readonly endTime: number | null | undefined;
    readonly id: string;
    readonly startTime: number;
  };
};
export type ActivityEditQuery = {
  response: ActivityEditQuery$data;
  variables: ActivityEditQuery$variables;
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
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "period",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "startTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "endTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Category",
        "kind": "LinkedField",
        "name": "category",
        "plural": false,
        "selections": [
          (v1/*: any*/),
          (v2/*: any*/)
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "categories",
    "plural": true,
    "selections": [
      (v1/*: any*/),
      (v2/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityEditQuery",
    "selections": (v3/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ActivityEditQuery",
    "selections": (v3/*: any*/)
  },
  "params": {
    "cacheID": "205cd2072655830750369170b7175610",
    "id": null,
    "metadata": {},
    "name": "ActivityEditQuery",
    "operationKind": "query",
    "text": "query ActivityEditQuery(\n  $id: ID!\n) {\n  period(id: $id) {\n    id\n    startTime\n    endTime\n    category {\n      id\n      name\n    }\n  }\n  categories {\n    id\n    name\n    enabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "05fa2a419ec0be1c3b3547165158e7f4";

export default node;
