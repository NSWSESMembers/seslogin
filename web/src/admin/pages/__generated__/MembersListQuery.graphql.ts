/**
 * @generated SignedSource<<dbb190081ec06b860a15f1173231d14b>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersListQuery$variables = {
  location: string;
};
export type MembersListQuery$data = {
  readonly location: {
    readonly id: string;
    readonly lastSuccessfulMemberSync: number | null | undefined;
    readonly people: ReadonlyArray<{
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
      readonly memberNumber: string | null | undefined;
      readonly missingSince: number | null | undefined;
      readonly sesApiPersonId: string | null | undefined;
    }>;
    readonly sesApiHeadquartersId: string | null | undefined;
  };
};
export type MembersListQuery = {
  response: MembersListQuery$data;
  variables: MembersListQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "location"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "sesApiHeadquartersId",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastSuccessfulMemberSync",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "people",
        "plural": true,
        "selections": [
          (v1/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "firstName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "memberNumber",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "sesApiPersonId",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "missingSince",
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
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "MembersListQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "MembersListQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "af553320c4193d43740cf973dd2ffbda",
    "id": null,
    "metadata": {},
    "name": "MembersListQuery",
    "operationKind": "query",
    "text": "query MembersListQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    sesApiHeadquartersId\n    lastSuccessfulMemberSync\n    people {\n      id\n      firstName\n      lastName\n      memberNumber\n      sesApiPersonId\n      missingSince\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "a8cb1b33bcbbb8d8c1b78a3c88fc9548";

export default node;
