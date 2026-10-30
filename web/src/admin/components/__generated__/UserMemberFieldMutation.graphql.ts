/**
 * @generated SignedSource<<44c28bd8ce156dbc362fe21cc5bb6bdc>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type UserMemberFieldMutation$variables = {
  personId?: string | null | undefined;
  userId: string;
};
export type UserMemberFieldMutation$data = {
  readonly setUserMember: {
    readonly " $fragmentSpreads": FragmentRefs<"UserMemberField_user">;
  };
};
export type UserMemberFieldMutation = {
  response: UserMemberFieldMutation$data;
  variables: UserMemberFieldMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "personId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "userId"
},
v2 = [
  {
    "kind": "Variable",
    "name": "personId",
    "variableName": "personId"
  },
  {
    "kind": "Variable",
    "name": "userId",
    "variableName": "userId"
  }
],
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "UserMemberFieldMutation",
    "selections": [
      {
        "alias": null,
        "args": (v2/*:: as any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "setUserMember",
        "plural": false,
        "selections": [
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "UserMemberField_user"
          }
        ],
        "storageKey": null
      }
    ],
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "UserMemberFieldMutation",
    "selections": [
      {
        "alias": null,
        "args": (v2/*:: as any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "setUserMember",
        "plural": false,
        "selections": [
          (v3/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v3/*:: as any*/),
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
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v3/*:: as any*/),
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
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "c92da6d8c84999f0d9353ea2b3ab5995",
    "id": null,
    "metadata": {},
    "name": "UserMemberFieldMutation",
    "operationKind": "mutation",
    "text": "mutation UserMemberFieldMutation(\n  $userId: ID!\n  $personId: ID\n) {\n  setUserMember(userId: $userId, personId: $personId) {\n    ...UserMemberField_user\n    id\n  }\n}\n\nfragment UserMemberField_user on User {\n  id\n  person {\n    id\n    firstName\n    lastName\n    memberNumber\n    location {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "9b08a6b3d12508a2aefa7366f1d5fd64";

export default node;
