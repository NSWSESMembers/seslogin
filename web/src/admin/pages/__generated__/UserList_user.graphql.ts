/**
 * @generated SignedSource<<570f6f1b98819fce8b34d8408e53261c>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type UserList_user$data = {
  readonly accessTime: number | null | undefined;
  readonly email: string;
  readonly enabled: boolean;
  readonly id: string;
  readonly isDev: boolean;
  readonly isSuper: boolean;
  readonly locationGrantIds: ReadonlyArray<string>;
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
  readonly readOnlyLocationGrantIds: ReadonlyArray<string>;
  readonly " $fragmentType": "UserList_user";
};
export type UserList_user$key = {
  readonly " $data"?: UserList_user$data;
  readonly " $fragmentSpreads": FragmentRefs<"UserList_user">;
};

const node: ReaderFragment = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
};
return {
  "argumentDefinitions": [],
  "kind": "Fragment",
  "metadata": {
    "throwOnFieldError": true
  },
  "name": "UserList_user",
  "selections": [
    (v0/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "email",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "accessTime",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "isSuper",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "isDev",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "locationGrantIds",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "readOnlyLocationGrantIds",
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
        (v0/*:: as any*/),
        {
          "alias": null,
          "args": null,
          "kind": "ScalarField",
          "name": "name",
          "storageKey": null
        }
      ],
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "enabled",
      "storageKey": null
    }
  ],
  "type": "User",
  "abstractKey": null
};
})();

(node as any).hash = "dfe4d7a6b9293f9117f794e84f8ff375";

export default node;
