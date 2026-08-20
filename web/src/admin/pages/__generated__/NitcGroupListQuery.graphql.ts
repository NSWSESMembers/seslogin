/**
 * @generated SignedSource<<8c20d2507d8a0d944af489b76bf8908a>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type NitcGroupListQuery$variables = Record<PropertyKey, never>;
export type NitcGroupListQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
    readonly nitcGroupId: string | null | undefined;
  }>;
  readonly nitcGroups: ReadonlyArray<{
    readonly id: string;
    readonly nitcType: string;
    readonly sesTags: ReadonlyArray<{
      readonly id: string;
      readonly name: string;
    }>;
  }>;
};
export type NitcGroupListQuery = {
  response: NitcGroupListQuery$data;
  variables: NitcGroupListQuery$variables;
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
    "concreteType": "NitcGroup",
    "kind": "LinkedField",
    "name": "nitcGroups",
    "plural": true,
    "selections": [
      (v0/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcType",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "SesNonIncidentTag",
        "kind": "LinkedField",
        "name": "sesTags",
        "plural": true,
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
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "categories",
    "plural": true,
    "selections": [
      (v0/*: any*/),
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcGroupId",
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
    "name": "NitcGroupListQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "NitcGroupListQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "8ad8670b88ee9cb6d47c7184d616efa5",
    "id": null,
    "metadata": {},
    "name": "NitcGroupListQuery",
    "operationKind": "query",
    "text": "query NitcGroupListQuery {\n  nitcGroups {\n    id\n    nitcType\n    sesTags {\n      id\n      name\n    }\n  }\n  categories {\n    id\n    name\n    nitcGroupId\n  }\n}\n"
  }
};
})();

(node as any).hash = "a21b1803cb430156de805788f96c9b9b";

export default node;
