/**
 * @generated SignedSource<<ee40f0e3d8308385aa3a734c0c27cea9>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type RegisterState = "NOT_FOUND" | "SIGNED_IN" | "SIGN_OUT_PENDING" | "%future added value";
export type ScanControllerRegister2Mutation$variables = {
  memberNumber: string;
};
export type ScanControllerRegister2Mutation$data = {
  readonly scanRegister2: {
    readonly awardedBadges: ReadonlyArray<{
      readonly description: string;
      readonly id: string;
      readonly name: string;
      readonly tier: string;
    }>;
    readonly period: {
      readonly endTime: number | null | undefined;
      readonly id: string;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      };
      readonly startTime: number;
    } | null | undefined;
    readonly state: RegisterState;
  };
};
export type ScanControllerRegister2Mutation = {
  response: ScanControllerRegister2Mutation$data;
  variables: ScanControllerRegister2Mutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "memberNumber"
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
        "name": "memberNumber",
        "variableName": "memberNumber"
      }
    ],
    "concreteType": "RegisterResult",
    "kind": "LinkedField",
    "name": "scanRegister2",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "state",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "BadgeAward",
        "kind": "LinkedField",
        "name": "awardedBadges",
        "plural": true,
        "selections": [
          (v1/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "description",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "tier",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
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
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
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
              }
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
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanControllerRegister2Mutation",
    "selections": (v2/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ScanControllerRegister2Mutation",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "8e71c1f3a8b853259adb8840b657aaa9",
    "id": null,
    "metadata": {},
    "name": "ScanControllerRegister2Mutation",
    "operationKind": "mutation",
    "text": "mutation ScanControllerRegister2Mutation(\n  $memberNumber: String!\n) {\n  scanRegister2(memberNumber: $memberNumber) {\n    state\n    awardedBadges {\n      id\n      name\n      description\n      tier\n    }\n    period {\n      id\n      startTime\n      endTime\n      person {\n        id\n        firstName\n        lastName\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "608c0050e4ed27b179a995c14d4f3831";

export default node;
