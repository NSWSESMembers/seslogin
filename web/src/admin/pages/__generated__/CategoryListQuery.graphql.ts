/**
 * @generated SignedSource<<d9a9365ac96a26b3c8e43344d76af562>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type CategoryListQuery$variables = Record<PropertyKey, never>;
export type CategoryListQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly enabled: boolean;
    readonly id: string;
    readonly isVirtual: boolean;
    readonly name: string;
    readonly nitcGroup: {
      readonly id: string;
      readonly nitcType: string;
      readonly sesTags: ReadonlyArray<{
        readonly id: string;
        readonly name: string;
      }>;
    } | null | undefined;
    readonly nitcGroupId: string | null | undefined;
    readonly nitcParticipantType: string | null | undefined;
  }>;
};
export type CategoryListQuery = {
  response: CategoryListQuery$data;
  variables: CategoryListQuery$variables;
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
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "categories",
    "plural": true,
    "selections": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isVirtual",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcGroupId",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcParticipantType",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "NitcGroup",
        "kind": "LinkedField",
        "name": "nitcGroup",
        "plural": false,
        "selections": [
          (v0/*:: as any*/),
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
              (v0/*:: as any*/),
              (v1/*:: as any*/)
            ],
            "storageKey": null
          }
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "CategoryListQuery",
    "selections": (v2/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "CategoryListQuery",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "c80d6fb923b0f673592e455b27b66761",
    "id": null,
    "metadata": {},
    "name": "CategoryListQuery",
    "operationKind": "query",
    "text": "query CategoryListQuery {\n  categories {\n    id\n    name\n    enabled\n    isVirtual\n    nitcGroupId\n    nitcParticipantType\n    nitcGroup {\n      id\n      nitcType\n      sesTags {\n        id\n        name\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "e098070fb353b6842ef6fab63b344c3d";

export default node;
