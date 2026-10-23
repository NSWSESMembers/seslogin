/**
 * @generated SignedSource<<c784635f4cbdaf78d3155846289f9b87>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ReaderFragment } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type UserMemberField_user$data = {
  readonly id: string;
  readonly person: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly location: {
      readonly id: string;
      readonly name: string;
    };
    readonly memberNumber: string | null | undefined;
  } | null | undefined;
  readonly " $fragmentType": "UserMemberField_user";
};
export type UserMemberField_user$key = {
  readonly " $data"?: UserMemberField_user$data;
  readonly " $fragmentSpreads": FragmentRefs<"UserMemberField_user">;
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
  "metadata": null,
  "name": "UserMemberField_user",
  "selections": [
    (v0/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "concreteType": "Person",
      "kind": "LinkedField",
      "name": "person",
      "plural": false,
      "selections": [
        (v0/*:: as any*/),
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
        }
      ],
      "storageKey": null
    }
  ],
  "type": "User",
  "abstractKey": null
};
})();

(node as any).hash = "d15d02537f5aaf17f23ed820ddc2fe05";

export default node;
